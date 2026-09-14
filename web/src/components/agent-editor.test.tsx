vi.mock("@/components/agent-iam", () => ({ AgentIam: () => <p>Agent access</p> }));
import { create } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  AgentSchema,
  BinaryPermission,
  TargetSelection,
} from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { AgentEditor } from "./agent-editor";

const updateAgent = vi.hoisted(() => vi.fn());
const createAgent = vi.hoisted(() => vi.fn());
const getDeployment = vi.hoisted(() => vi.fn());
const setDeployment = vi.hoisted(() => vi.fn());
vi.mock("@/client", () => ({
  agents: { updateAgent, createAgent },
  deployments: { getDeployment, setDeployment },
}));
vi.mock("./agent-connections", () => ({
  AgentConnections: () => <p>Connected chat providers</p>,
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const agent = create(AgentSchema, {
  id: "agent-ada",
  name: "Ada",
  endpointUrl: "https://ada.example.com",
});

it("autosaves typed capabilities and keeps header saves independent", async () => {
  updateAgent.mockImplementation(async (patch) => ({ agent: { ...agent, ...patch } }));
  const onSaved = vi.fn();
  render(<AgentEditor agent={agent} onClose={vi.fn()} onSaved={onSaved} />);
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "Read agents" })).getByRole("tab", { name: "All" }),
  );
  await waitFor(() =>
    expect(updateAgent).toHaveBeenLastCalledWith(
      expect.objectContaining({
        capabilities: expect.objectContaining({
          agentsRead: expect.objectContaining({ mode: TargetSelection.ALL }),
        }),
      }),
    ),
  );
  await screen.findByRole("status", { name: "Read agents saved" });
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "Create agents" })).getByRole("tab", {
      name: "Yes",
    }),
  );
  await waitFor(() =>
    expect(updateAgent).toHaveBeenLastCalledWith(
      expect.objectContaining({
        capabilities: expect.objectContaining({
          agentsCreate: BinaryPermission.YES,
          agentsRead: expect.objectContaining({ mode: TargetSelection.ALL }),
        }),
      }),
    ),
  );
  await screen.findByRole("status", { name: "Create agents saved" });
  const savedCalls = updateAgent.mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: "Edit agent name" }));
  expect(updateAgent).toHaveBeenCalledTimes(savedCalls);
  fireEvent.change(screen.getByRole("textbox", { name: "Agent name" }), {
    target: { value: "Ada Lovelace" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByRole("heading", { name: "Ada Lovelace" });
  expect(updateAgent).toHaveBeenLastCalledWith({ id: agent.id, name: "Ada Lovelace" });
  expect(onSaved).not.toHaveBeenCalled();

  getDeployment.mockResolvedValue({
    deployment: { agentId: agent.id, mode: 1, failureMode: 1, routing: 1, servingDeploymentId: "" },
    deployments: [],
    instances: [],
  });
  setDeployment.mockImplementation(async (values) => ({ deployment: values }));
  fireEvent.click(screen.getByRole("tab", { name: "Deployment" }));
  fireEvent.click(await screen.findByRole("tab", { name: "Manual" }));
  fireEvent.click(screen.getByRole("button", { name: "Save deployment" }));
  await screen.findByText("Deployment saved.");
  expect(setDeployment).toHaveBeenLastCalledWith(
    expect.objectContaining({ agentId: agent.id, routing: 2 }),
  );

  fireEvent.click(screen.getByRole("tab", { name: "Chat providers" }));
  expect(screen.getByRole("tabpanel").textContent).toContain("Connected chat providers");
  fireEvent.click(screen.getByRole("tab", { name: "Capabilities" }));
  expect(
    within(screen.getByRole("tablist", { name: "Read agents" }))
      .getByRole("tab", { name: "All" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  expect(screen.queryByRole("button", { name: "Save changes" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
});

it("keeps failed edits available for retry without a redundant back button", async () => {
  updateAgent
    .mockRejectedValueOnce(new Error("Unable to reach the server"))
    .mockResolvedValueOnce({ agent: { ...agent, name: "Ada Lovelace" } });
  const onSaved = vi.fn();
  const onClose = vi.fn();
  render(<AgentEditor agent={agent} onClose={onClose} onSaved={onSaved} />);
  fireEvent.click(screen.getByRole("button", { name: "Edit agent name" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Agent name" }), {
    target: { value: "Ada Lovelace" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toBe("Unable to reach the server");
  expect((screen.getByRole("textbox", { name: "Agent name" }) as HTMLInputElement).value).toBe(
    "Ada Lovelace",
  );
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByRole("heading", { name: "Ada Lovelace" });
  expect(onSaved).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Back to agents" })).toBeNull();
  expect(onClose).not.toHaveBeenCalled();
});

it("requires the header endpoint when submitting the wide create form", async () => {
  createAgent.mockResolvedValue({ agent });
  const onSaved = vi.fn();
  render(<AgentEditor agent={null} onClose={vi.fn()} onSaved={onSaved} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Agent name" }), {
    target: { value: "Ada" },
  });
  fireEvent.change(screen.getByLabelText("Webhook signing key"), {
    target: { value: "a-caller-generated-secret-with-32-bytes" },
  });
  expect((screen.getByRole("button", { name: "Create agent" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  const endpoint = screen.getByRole("textbox", { name: "Agent endpoint" }) as HTMLInputElement;
  expect(endpoint.required).toBe(true);
  expect(endpoint.form?.id).toBe("agent-settings");
  fireEvent.change(endpoint, { target: { value: "https://ada.example.com" } });
  fireEvent.click(screen.getByRole("button", { name: "Create agent" }));
  await waitFor(() =>
    expect(createAgent).toHaveBeenCalledWith(
      expect.objectContaining({ name: "Ada", endpointUrl: "https://ada.example.com" }),
    ),
  );
  await waitFor(() => expect(onSaved).toHaveBeenCalledWith(true));
});

it("rolls back a failed automatic capability update and permits retry", async () => {
  updateAgent
    .mockRejectedValueOnce(new Error("Capability update failed"))
    .mockImplementation(async (patch) => ({ agent: { ...agent, ...patch } }));
  render(<AgentEditor agent={agent} onClose={vi.fn()} onSaved={vi.fn()} />);
  const options = () => within(screen.getByRole("tablist", { name: "Create agents" }));
  fireEvent.click(options().getByRole("tab", { name: "Yes" }));
  expect((await screen.findByRole("status", { name: "Create agents failed to save" })).title).toBe(
    "Capability update failed",
  );
  expect(options().getByRole("tab", { name: "No" }).getAttribute("aria-selected")).toBe("true");
  fireEvent.click(options().getByRole("tab", { name: "Yes" }));
  await waitFor(() => expect(updateAgent).toHaveBeenCalledTimes(2));
  await screen.findByRole("status", { name: "Create agents saved" });
  expect(options().getByRole("tab", { name: "Yes" }).getAttribute("aria-selected")).toBe("true");
});

it("shows saving feedback beside only the capability being persisted", async () => {
  let finish!: (response: { agent: typeof agent }) => void;
  updateAgent.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  render(<AgentEditor agent={agent} onClose={vi.fn()} onSaved={vi.fn()} />);
  fireEvent.click(
    within(screen.getByRole("tablist", { name: "Read agents" })).getByRole("tab", { name: "All" }),
  );
  const heading = screen.getByRole("heading", { name: "Read agents" });
  expect(
    within(heading.parentElement!).getByRole("status", { name: "Saving Read agents" }),
  ).toBeTruthy();
  expect(screen.queryByRole("status", { name: "Saving Create agents" })).toBeNull();
  expect(screen.queryByText("Saving capability…")).toBeNull();
  expect(screen.getByText("Saving Read agents").className).toContain("sr-only");
  await act(async () => {
    finish({ agent: { ...agent, capabilities: updateAgent.mock.lastCall![0].capabilities } });
  });
  await screen.findByRole("status", { name: "Read agents saved" });
});
