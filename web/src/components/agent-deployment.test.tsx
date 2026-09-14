import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  AgentDeploymentSchema,
  AgentInstanceSchema,
  DeploymentMode,
  DeploymentRouting,
  DeploymentSchema,
  DeploymentSource,
  DeploymentStatus,
  DeploymentTarget,
  SidecarFailureMode,
} from "@trytilde/contracts/tilde/types/v1/deployment_pb.js";
import { AgentDeployment } from "./agent-deployment";

const rpc = vi.hoisted(() => ({
  getDeployment: vi.fn(),
  setDeployment: vi.fn(),
  registerDeployment: vi.fn(),
  promoteDeployment: vi.fn(),
  retireDeployment: vi.fn(),
  issueDeploymentToken: vi.fn(),
}));
vi.mock("@/client", () => ({ deployments: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

const settings = create(DeploymentSchema, {
  agentId: "agent-1",
  mode: DeploymentMode.GATEWAY,
  failureMode: SidecarFailureMode.REASSIGN,
  routing: DeploymentRouting.MANUAL,
  servingDeploymentId: "dep-1",
});
const servingDeployment = create(AgentDeploymentSchema, {
  id: "dep-1",
  agentId: "agent-1",
  source: DeploymentSource.CI,
  target: DeploymentTarget.DIRECT,
  endpointUrl: "https://ada.example.com",
  repository: "acme/ada",
  commitSha: "abcdef1234567890",
  label: "Release 1",
  status: DeploymentStatus.REGISTERED,
  tokenIssued: true,
  serving: true,
  createdAt: { seconds: 1_700_000_000n },
});
const candidate = create(AgentDeploymentSchema, {
  id: "dep-2",
  agentId: "agent-1",
  source: DeploymentSource.MANUAL,
  target: DeploymentTarget.DIRECT,
  endpointUrl: "https://ada-next.example.com",
  commitSha: "1234567abcdef",
  status: DeploymentStatus.REGISTERED,
  createdAt: { seconds: 1_700_000_100n },
});
const instance = create(AgentInstanceSchema, {
  instanceId: "inst-1",
  agentId: "agent-1",
  deploymentId: "dep-1",
  ready: true,
});

it("renders the serving summary and deployment rows with their instances", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: settings,
    deployments: [servingDeployment, candidate],
    instances: [instance],
  });
  render(<AgentDeployment agentId="agent-1" paused={false} />);
  expect((await screen.findByTestId("serving-summary")).textContent).toContain("Serving Release 1");
  const rows = screen.getAllByRole("listitem", { name: /^Deployment / });
  expect(rows[0].getAttribute("aria-label")).toBe("Deployment 1234567");
  const row = screen.getByRole("listitem", { name: "Deployment Release 1" });
  expect(within(row).getByText("abcdef1 · acme/ada")).toBeTruthy();
  expect(within(row).getByText("CI")).toBeTruthy();
  expect(within(row).getByText("Direct")).toBeTruthy();
  expect(within(row).getByText("Registered")).toBeTruthy();
  expect(within(row).getByText("Serving")).toBeTruthy();
  expect(within(row).getByText("https://ada.example.com")).toBeTruthy();
  expect(within(row).getByText("inst-1")).toBeTruthy();
  expect(within(row).getByText("Ready")).toBeTruthy();
  expect(within(row).queryByRole("button", { name: "Promote Release 1" })).toBeNull();
  expect(screen.getByRole("tab", { name: "Sidecar" }).getAttribute("aria-disabled")).toBe("true");
  expect(screen.getByRole("tab", { name: "Manual" }).getAttribute("aria-selected")).toBe("true");
});

it("registers a manual deployment and shows the token once", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: { ...settings, servingDeploymentId: "" },
    deployments: [],
    instances: [],
  });
  rpc.registerDeployment.mockResolvedValue({
    deployment: { ...candidate, label: "Canary" },
    token: "tok_secret",
    created: true,
  });
  render(<AgentDeployment agentId="agent-1" paused={false} />);
  expect((await screen.findByTestId("serving-summary")).textContent).toContain(
    "No deployment is serving",
  );
  fireEvent.change(screen.getByLabelText("Agent endpoint URL"), {
    target: { value: "https://ada-next.example.com" },
  });
  fireEvent.change(screen.getByLabelText("Label"), { target: { value: "Canary" } });
  fireEvent.change(screen.getByLabelText("Commit SHA"), { target: { value: "1234567abcdef" } });
  fireEvent.click(screen.getByRole("button", { name: "Register deployment" }));
  await screen.findByText("Deployment registered.");
  expect(rpc.registerDeployment).toHaveBeenCalledWith({
    agentId: "agent-1",
    source: DeploymentSource.MANUAL,
    target: DeploymentTarget.DIRECT,
    endpointUrl: "https://ada-next.example.com",
    targetReference: undefined,
    label: "Canary",
    repository: undefined,
    commitSha: "1234567abcdef",
  });
  expect((screen.getByLabelText("Copy this token now for Canary") as HTMLInputElement).value).toBe(
    "tok_secret",
  );
  expect(screen.getByText(/Give this token to the deployed agent/)).toBeTruthy();
  expect(rpc.getDeployment).toHaveBeenCalledTimes(2);
});

it("promotes a deployment and refreshes", async () => {
  let servingId = "dep-1";
  rpc.getDeployment.mockImplementation(async () => ({
    deployment: { ...settings, servingDeploymentId: servingId },
    deployments: [
      { ...servingDeployment, serving: servingId === "dep-1" },
      { ...candidate, serving: servingId === "dep-2" },
    ],
    instances: [],
  }));
  rpc.promoteDeployment.mockImplementation(async ({ deploymentId }) => {
    servingId = deploymentId;
    return { deployment: { ...settings, servingDeploymentId: deploymentId } };
  });
  render(<AgentDeployment agentId="agent-1" paused={false} />);
  fireEvent.click(await screen.findByRole("button", { name: "Promote 1234567" }));
  await screen.findByText("1234567 is now serving.");
  expect(rpc.promoteDeployment).toHaveBeenCalledWith({ agentId: "agent-1", deploymentId: "dep-2" });
  await waitFor(() => expect(rpc.getDeployment).toHaveBeenCalledTimes(2));
  expect(screen.getByTestId("serving-summary").textContent).toContain("Serving 1234567");
  expect(screen.queryByRole("button", { name: "Promote 1234567" })).toBeNull();
  expect(screen.getByRole("button", { name: "Promote Release 1" })).toBeTruthy();
});

it("saves settings and shows the sidecar snippet for sidecar tokens", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: { ...settings, mode: DeploymentMode.SIDECAR, servingDeploymentId: "" },
    deployments: [
      { ...candidate, target: DeploymentTarget.SIDECAR, endpointUrl: "", label: "Replica set" },
    ],
    instances: [],
  });
  rpc.setDeployment.mockImplementation(async (values) => ({ deployment: values }));
  rpc.issueDeploymentToken.mockResolvedValue({ token: "tok_rotated" });
  render(<AgentDeployment agentId="agent-1" paused />);
  fireEvent.click(await screen.findByRole("tab", { name: "Latest" }));
  fireEvent.click(screen.getByRole("tab", { name: "Stop" }));
  fireEvent.click(screen.getByRole("button", { name: "Save deployment" }));
  await screen.findByText("Deployment saved.");
  expect(rpc.setDeployment).toHaveBeenCalledWith({
    agentId: "agent-1",
    mode: DeploymentMode.SIDECAR,
    failureMode: SidecarFailureMode.STOP,
    routing: DeploymentRouting.LATEST,
  });
  fireEvent.click(screen.getByRole("button", { name: "Rotate token for Replica set" }));
  await screen.findByText("Token rotated for Replica set.");
  expect(rpc.issueDeploymentToken).toHaveBeenCalledWith({
    agentId: "agent-1",
    deploymentId: "dep-2",
  });
  expect(screen.getByText(/ENGINE_SIDECAR_AGENT_TOKENS=tok_rotated/)).toBeTruthy();
  expect(
    screen.getByText(/connectAgent\(\{ gatewayUrl: "http:\/\/127\.0\.0\.1:8081\/agents\/agent-1"/),
  ).toBeTruthy();
  expect(screen.queryByLabelText("Agent endpoint URL")).toBeNull();
  expect(
    within(screen.getByLabelText("Target") as HTMLSelectElement)
      .getAllByRole("option")
      .map((option) => option.textContent),
  ).toEqual(["Sidecar"]);
});
