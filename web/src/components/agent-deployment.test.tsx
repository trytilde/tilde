import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  AgentDeploymentSchema,
  AgentInstanceSchema,
  DeploymentRouting,
  DeploymentSchema,
  DeploymentSource,
  DeploymentStatus,
  DeploymentTarget,
  SidecarFailureMode,
} from "@trytilde/contracts/tilde/types/v1/deployment_pb.js";
import { AgentDeployment, commitUrl } from "./agent-deployment";

const rpc = vi.hoisted(() => ({
  getDeployment: vi.fn(),
  setDeployment: vi.fn(),
  setDeploymentWeights: vi.fn(),
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

  failureMode: SidecarFailureMode.REASSIGN,
  routing: DeploymentRouting.WEIGHTED,
  servingDeploymentId: "dep-1",
});
const servingDeployment = create(AgentDeploymentSchema, {
  id: "dep-1",
  agentId: "agent-1",
  source: DeploymentSource.CI,
  target: DeploymentTarget.GATEWAY,
  repository: "acme/ada",
  commitSha: "abcdef1234567890",
  commitMessage: "Ship the onboarding flow\n\nLonger body that stays out of the table.",
  branch: "main",
  commitAuthor: "ada",
  label: "Release 1",
  status: DeploymentStatus.REGISTERED,
  tokenIssued: true,
  serving: true,
  routable: true,
  trafficWeight: 100,
  createdAt: { seconds: 1_700_000_000n },
});
const candidate = create(AgentDeploymentSchema, {
  id: "dep-2",
  routable: true,
  agentId: "agent-1",
  source: DeploymentSource.MANUAL,
  target: DeploymentTarget.GATEWAY,
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

it("renders deployment rows with their serving status and instances", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: settings,
    deployments: [servingDeployment, candidate],
    instances: [instance],
  });
  render(<AgentDeployment agentId="agent-1" />);
  const table = await screen.findByRole("table", { name: "Deployments" });
  expect(
    within(table)
      .getAllByRole("columnheader")
      .map((header) => header.textContent),
  ).toEqual([
    "Deployment",
    "Traffic weight",
    "Type",
    "Status",
    "Commit",
    "Branch",
    "Created",
    "Actions",
  ]);
  const rows = within(table)
    .getAllByRole("row")
    .filter((row) => row.hasAttribute("aria-label"));
  expect(rows[0].getAttribute("aria-label")).toBe("Deployment 1234567");
  const row = screen.getByRole("row", { name: "Deployment Ship the onboarding flow" });
  expect(within(row).getByText("Ship the onboarding flow")).toBeTruthy();
  expect(within(row).queryByText("Release 1")).toBeNull();
  const commit = within(row).getByRole("link", { name: "abcdef1" });
  expect(commit.getAttribute("href")).toBe("https://github.com/acme/ada/commit/abcdef1234567890");
  expect(commit.getAttribute("target")).toBe("_blank");
  expect(commit.getAttribute("rel")).toBe("noopener noreferrer");
  expect(within(row).getByText("main")).toBeTruthy();
  expect(within(row).getByText("Gateway")).toBeTruthy();
  expect(within(row).getByText("Serving (1)")).toBeTruthy();
  expect(within(row).queryByText("1/1 ready")).toBeNull();
  expect(within(row).queryByText("https://ada.example.com")).toBeNull();
  // The manual candidate has no git provider details: plain commit, no link, no branch.
  expect(within(rows[0]).queryByRole("link")).toBeNull();
  expect(within(rows[0]).getAllByText("1234567").length).toBeGreaterThan(0);
  expect(within(rows[0]).getByText("Standby (0)")).toBeTruthy();
  expect(within(rows[0]).queryByText("No instances")).toBeNull();
  fireEvent.click(
    within(row).getByRole("button", { name: "Actions for Ship the onboarding flow" }),
  );
  await screen.findByRole("menuitem", { name: "Retire" });
  expect(screen.queryByRole("menuitem", { name: "Promote" })).toBeNull();
  expect(screen.getByRole("tab", { name: "Weighted" }).getAttribute("aria-selected")).toBe("true");
});

it("links commits on full repository URLs and skips unknown providers", () => {
  expect(commitUrl("https://gitlab.example.com/acme/ada.git", "abc")).toBe(
    "https://gitlab.example.com/acme/ada/commit/abc",
  );
  expect(commitUrl("acme/ada", "")).toBeUndefined();
  expect(commitUrl("not a repository", "abc")).toBeUndefined();
});

it("keeps registration in a modal opened from the deployments toolbar", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: { ...settings, servingDeploymentId: "" },
    deployments: [],
    instances: [],
  });
  render(<AgentDeployment agentId="agent-1" />);
  await screen.findByRole("table", { name: "Deployments" });
  expect(screen.getByRole("table", { name: "Deployments" })).toBeTruthy();
  expect(screen.getByText("No deployments registered yet.")).toBeTruthy();
  expect(
    within(screen.getByRole("toolbar", { name: "Deployment actions" })).getByRole("button", {
      name: "Register deployment",
    }),
  ).toBeTruthy();
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.queryByRole("button", { name: "Save deployment" })).toBeNull();
  expect(screen.queryByLabelText("Agent endpoint URL")).toBeNull();
  expect(rpc.setDeployment).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Register deployment" }));
  await screen.findByRole("dialog", { name: "Register deployment" });
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(rpc.registerDeployment).not.toHaveBeenCalled();
});

it("promotes a deployment and refreshes", async () => {
  let servingId = "dep-1";
  rpc.getDeployment.mockImplementation(async () => ({
    deployment: { ...settings, routing: DeploymentRouting.LATEST, servingDeploymentId: servingId },
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
  render(<AgentDeployment agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "Actions for 1234567" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Promote" }));
  await screen.findByText("1234567 is now serving.");
  expect(rpc.promoteDeployment).toHaveBeenCalledWith({ agentId: "agent-1", deploymentId: "dep-2" });
  await waitFor(() => expect(rpc.getDeployment).toHaveBeenCalledTimes(2));
  expect(
    within(screen.getByRole("row", { name: "Deployment 1234567" })).getByText("Serving (0)"),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Actions for 1234567" }));
  await screen.findByRole("menuitem", { name: "Retire" });
  expect(screen.queryByRole("menuitem", { name: "Promote" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Actions for Ship the onboarding flow" }));
  await screen.findByRole("menuitem", { name: "Promote" });
});

it("autosaves settings and shows the sidecar snippet for sidecar tokens", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: { ...settings, servingDeploymentId: "" },
    deployments: [{ ...candidate, target: DeploymentTarget.SIDECAR, label: "Replica set" }],
    instances: [],
  });
  rpc.setDeployment.mockImplementation(async (values) => ({ deployment: values }));
  rpc.issueDeploymentToken.mockResolvedValue({ token: "tok_rotated" });
  render(<AgentDeployment agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("tab", { name: "Latest" }));
  await screen.findByRole("status", { name: "Routing saved" });
  fireEvent.click(screen.getByRole("tab", { name: "Stop" }));
  await screen.findByRole("status", { name: "Failure mode saved" });
  expect(rpc.setDeployment).toHaveBeenLastCalledWith(
    {
      agentId: "agent-1",

      failureMode: SidecarFailureMode.STOP,
      routing: DeploymentRouting.LATEST,
    },
    { signal: expect.any(AbortSignal) },
  );
  expect(rpc.setDeployment).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button", { name: "Actions for Replica set" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Rotate token" }));
  await screen.findByRole("alertdialog", { name: "Rotate deployment token?" });
  expect(rpc.issueDeploymentToken).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Rotate token" }));
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
});

it("shows field feedback, rolls back failed saves, and updates serving rows after retry", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: settings,
    deployments: [servingDeployment, candidate],
    instances: [],
  });
  let rejectSave!: (error: Error) => void;
  rpc.setDeployment
    .mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectSave = reject;
        }),
    )
    .mockImplementation(async (values) => ({
      deployment: { ...settings, ...values, servingDeploymentId: candidate.id },
    }));
  render(<AgentDeployment agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("tab", { name: "Latest" }));
  await screen.findByRole("status", { name: "Saving Routing" });
  expect(screen.getByRole("tab", { name: "Weighted" }).getAttribute("aria-disabled")).toBe("true");
  fireEvent.click(screen.getByRole("tab", { name: "Weighted" }));
  expect(rpc.setDeployment).toHaveBeenCalledTimes(1);
  await act(async () => rejectSave(new Error("Routing could not be changed.")));
  await screen.findByRole("status", { name: "Routing failed to save" });
  expect(screen.getByRole("tab", { name: "Weighted" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getByRole("alert").textContent).toBe("Routing could not be changed.");
  fireEvent.click(screen.getByRole("tab", { name: "Latest" }));
  await screen.findByRole("status", { name: "Routing saved" });
  expect(screen.queryByRole("alert")).toBeNull();
  expect(
    within(screen.getByRole("row", { name: "Deployment 1234567" })).getByText("Serving (0)"),
  ).toBeTruthy();
  expect(
    within(screen.getByRole("row", { name: "Deployment Ship the onboarding flow" })).getByText(
      "Standby (0)",
    ),
  ).toBeTruthy();
  expect(rpc.setDeployment).toHaveBeenLastCalledWith(
    {
      agentId: "agent-1",

      failureMode: SidecarFailureMode.REASSIGN,
      routing: DeploymentRouting.LATEST,
    },
    { signal: expect.any(AbortSignal) },
  );
});

it("validates traffic weights before confirmation and preserves drafts when canceled", async () => {
  const retired = create(AgentDeploymentSchema, {
    ...candidate,
    id: "retired",
    label: "Retired release",
    routable: false,
    status: DeploymentStatus.RETIRED,
  });
  rpc.getDeployment.mockResolvedValue({
    deployment: settings,
    deployments: [servingDeployment, candidate, retired],
    instances: [],
  });
  rpc.setDeploymentWeights.mockImplementation(async ({ weights }) => ({
    deployment: settings,
    deployments: [servingDeployment, candidate, retired].map((row) => ({
      ...row,
      trafficWeight:
        weights.find((w: { deploymentId: string }) => w.deploymentId === row.id)?.weight ?? 0,
    })),
  }));
  render(<AgentDeployment agentId="agent-1" />);
  const first = await screen.findByRole("spinbutton", {
    name: "Traffic weight for Ship the onboarding flow",
  });
  const second = screen.getByRole("spinbutton", { name: "Traffic weight for 1234567" });
  expect(
    screen.queryByRole("spinbutton", { name: "Traffic weight for Retired release" }),
  ).toBeNull();
  expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
  fireEvent.change(first, { target: { value: "60" } });
  fireEvent.change(second, { target: { value: "20" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByRole("alertdialog", { name: "Unable to save traffic weights" });
  expect(screen.getByText("Traffic weights total 80%. They must add up to 100%.")).toBeTruthy();
  expect(rpc.setDeploymentWeights).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  fireEvent.change(second, { target: { value: "40" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByRole("alertdialog", { name: "Confirm traffic weights" });
  expect(rpc.setDeploymentWeights).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect((first as HTMLInputElement).value).toBe("60");
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  fireEvent.click(await screen.findByRole("button", { name: "Confirm changes" }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  expect(rpc.setDeploymentWeights).toHaveBeenCalledWith(
    {
      agentId: "agent-1",
      weights: expect.arrayContaining([
        { deploymentId: "dep-1", weight: 60 },
        { deploymentId: "dep-2", weight: 40 },
      ]),
    },
    { signal: expect.any(AbortSignal) },
  );
  expect(rpc.setDeploymentWeights.mock.calls[0][0].weights).toHaveLength(2);
  expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
});

it("rejects fractional weights and shows a server error without losing edited values", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: settings,
    deployments: [servingDeployment, candidate],
    instances: [],
  });
  rpc.setDeploymentWeights.mockRejectedValue(
    new Error("Deployments changed. Reload and try again."),
  );
  render(<AgentDeployment agentId="agent-1" />);
  const first = await screen.findByRole("spinbutton", {
    name: "Traffic weight for Ship the onboarding flow",
  });
  fireEvent.change(first, { target: { value: "99.5" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByRole("alertdialog", { name: "Unable to save traffic weights" });
  expect(screen.getByText(/Enter a whole percentage/)).toBeTruthy();
  expect(rpc.setDeploymentWeights).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  fireEvent.change(first, { target: { value: "75" } });
  fireEvent.change(screen.getByRole("spinbutton", { name: "Traffic weight for 1234567" }), {
    target: { value: "25" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  fireEvent.click(await screen.findByRole("button", { name: "Confirm changes" }));
  await screen.findByText("Deployments changed. Reload and try again.");
  expect((first as HTMLInputElement).value).toBe("75");
});

it("keeps disconnected deployments visible without offering weight inputs or promotion", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: settings,
    deployments: [servingDeployment, { ...candidate, routable: false }],
    instances: [instance],
  });
  render(<AgentDeployment agentId="agent-1" />);
  await screen.findByRole("table", { name: "Deployments" });
  const row = screen.getByRole("row", { name: "Deployment 1234567" });
  expect(within(row).getByText("Offline (0)")).toBeTruthy();
  expect(within(row).queryByRole("spinbutton")).toBeNull();
  expect(
    screen.getByRole("spinbutton", { name: "Traffic weight for Ship the onboarding flow" }),
  ).toBeTruthy();
});

it("shows the shortest unambiguous commit prefixes", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: { ...settings, routing: DeploymentRouting.LATEST },
    deployments: [
      { ...servingDeployment, commitSha: "abcdef1234567890" },
      { ...candidate, label: "Other", commitSha: "abcdef1934567890" },
    ],
    instances: [],
  });
  render(<AgentDeployment agentId="agent-1" />);
  await screen.findByRole("table", { name: "Deployments" });
  expect(screen.getByRole("link", { name: "abcdef12" })).toBeTruthy();
  expect(screen.getByText("abcdef19")).toBeTruthy();
  expect(screen.queryByText("abcdef1234567890")).toBeNull();
});

it("registers a gateway without a public URL and reveals its token once", async () => {
  let rows = [servingDeployment];
  rpc.getDeployment.mockImplementation(async () => ({
    deployment: settings,
    deployments: rows,
    instances: [],
  }));
  rpc.registerDeployment.mockImplementation(async (request) => {
    const deployment = create(AgentDeploymentSchema, { ...candidate, ...request, routable: false });
    rows = [...rows, deployment];
    return { deployment, token: "tok_new", created: true };
  });
  render(<AgentDeployment agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "Register deployment" }));
  const dialog = await screen.findByRole("dialog", { name: "Register deployment" });
  fireEvent.change(within(dialog).getByLabelText("Name"), { target: { value: "New release" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Register" }));
  await screen.findByRole("dialog", { name: "Deployment token" });
  expect(rpc.registerDeployment).toHaveBeenCalledWith({
    agentId: "agent-1",
    source: DeploymentSource.MANUAL,
    target: DeploymentTarget.GATEWAY,
    label: "New release",
  });
  expect(screen.getByDisplayValue("tok_new")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Done" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.queryByDisplayValue("tok_new")).toBeNull();
  expect(
    within(screen.getByRole("row", { name: "Deployment New release" })).getByText("Offline (0)"),
  ).toBeTruthy();
});

it("registers Lambda with an ARN and retains the form after an API error", async () => {
  rpc.getDeployment.mockResolvedValue({ deployment: settings, deployments: [], instances: [] });
  rpc.registerDeployment.mockRejectedValue(new Error("Function could not be registered."));
  render(<AgentDeployment agentId="agent-1" />);
  fireEvent.click(await screen.findByRole("button", { name: "Register deployment" }));
  await screen.findByRole("dialog", { name: "Register deployment" });
  fireEvent.click(screen.getByRole("combobox", { name: "Type" }));
  const lambda = await screen.findByRole("option", { name: "Lambda" });
  fireEvent.pointerDown(lambda, { pointerType: "mouse" });
  fireEvent.click(lambda);
  const arn = "arn:aws:lambda:eu-west-1:123456789012:function:example";
  fireEvent.change(await screen.findByLabelText("Function ARN"), { target: { value: arn } });
  expect(screen.queryByLabelText("Public URL (optional)")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Register" }));
  await screen.findByRole("alert");
  expect(rpc.registerDeployment).toHaveBeenCalledWith({
    agentId: "agent-1",
    source: DeploymentSource.MANUAL,
    target: DeploymentTarget.LAMBDA,
    label: "",
    targetReference: arn,
  });
  expect(screen.getByDisplayValue(arn)).toBeTruthy();
  expect(screen.getByRole("dialog", { name: "Register deployment" })).toBeTruthy();
});

it("requires confirmation for retirement and preserves the dialog on failure", async () => {
  rpc.getDeployment.mockResolvedValue({
    deployment: settings,
    deployments: [candidate],
    instances: [],
  });
  rpc.retireDeployment
    .mockRejectedValueOnce(new Error("Set the traffic weight to 0% first."))
    .mockResolvedValue({});
  render(<AgentDeployment agentId="agent-1" />);
  const actions = await screen.findByRole("button", { name: "Actions for 1234567" });
  fireEvent.click(actions);
  fireEvent.click(await screen.findByRole("menuitem", { name: "Retire" }));
  await screen.findByRole("alertdialog", { name: "Retire deployment?" });
  expect(rpc.retireDeployment).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  expect(rpc.retireDeployment).not.toHaveBeenCalled();
  fireEvent.click(actions);
  fireEvent.click(await screen.findByRole("menuitem", { name: "Retire" }));
  const modal = await screen.findByRole("alertdialog", { name: "Retire deployment?" });
  const confirm = within(modal).getByRole("button", { name: "Retire deployment" });
  expect(confirm.className).toContain("text-destructive");
  fireEvent.click(confirm);
  await within(modal).findByRole("alert");
  expect(within(modal).getByRole("alert").textContent).toBe("Set the traffic weight to 0% first.");
  fireEvent.click(confirm);
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  expect(rpc.retireDeployment).toHaveBeenCalledTimes(2);
  expect(rpc.retireDeployment).toHaveBeenLastCalledWith({
    agentId: "agent-1",
    deploymentId: "dep-2",
  });
});
