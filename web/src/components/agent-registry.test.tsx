vi.mock("@/components/agent-iam", () => ({ AgentIam: () => <p>Agent access</p> }));
import { useState } from "react";
import type { Agent } from "@/gen/tilde/types/v1/agent_pb.js";
import { AgentEditor } from "./agent-editor";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  AgentHealthStatus,
  BinaryPermission,
  TargetSelection,
} from "@/gen/tilde/types/v1/agent_pb.js";
import { AgentRegistry } from "./agent-registry";

const rpc = vi.hoisted(() => ({
  listAgents: vi.fn(),
  createAgent: vi.fn(),
  getAgent: vi.fn(),
  updateAgent: vi.fn(),
  deleteAgent: vi.fn(),
  pauseAgent: vi.fn(),
  resumeAgent: vi.fn(),
}));
vi.mock("@/client", () => ({ agents: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const ada = {
  id: "agent-ada",
  name: "Ada",
  endpointUrl: "https://ada.example.com",
  createdAt: { seconds: 1700000000n, nanos: 0 },
  updatedAt: { seconds: 1700000000n, nanos: 0 },
};
const grace = { ...ada, id: "agent-grace", name: "Grace" };

// Registry tests exercise selection through its navigation callback; route/history behavior
// is covered using the real TanStack route tree in router.test.tsx.
function RegistryHarness() {
  const [selected, setSelected] = useState<Agent>();
  return selected ? (
    <AgentEditor
      agent={selected}
      onClose={() => setSelected(undefined)}
      onSaved={() => setSelected(undefined)}
    />
  ) : (
    <AgentRegistry onOpen={setSelected} onCreate={() => {}} />
  );
}

describe("Agent Registry", () => {
  it("renders measured health, twelve hourly bars, and thread metrics", async () => {
    const healthHistory = Array.from({ length: 12 }, (_, index) => ({
      totalChecks: index < 7 ? 120 : 0,
      failedChecks: index < 2 ? 1 : 0,
      hourStart: { seconds: BigInt(1700000000 + index * 3600), nanos: 0 },
    }));
    rpc.listAgents.mockResolvedValue({
      agents: [
        {
          ...ada,
          metrics: {
            health: AgentHealthStatus.HEALTHY,
            healthHistory,
            threadCount: 12n,
            averageTurnsPerThread: 2.5,
            averageResponseMs: 1250,
          },
        },
      ],
      nextPageToken: "",
    });
    render(<RegistryHarness />);
    await screen.findByText("Healthy");
    expect(screen.getByText("12")).toBeTruthy();
    expect(screen.getByText("2.5")).toBeTruthy();
    expect(screen.getByText("1.3 s")).toBeTruthy();
    const chart = screen.getByRole("img", {
      name: "Last 12 hours: 5 healthy, 2 unhealthy, 5 with no data",
    });
    expect(chart.children.length).toBe(12);
    expect(chart.querySelectorAll('[data-health="unhealthy"]').length).toBe(2);
    expect(screen.queryByRole("columnheader", { name: "Endpoint" })).toBeNull();
  });

  it("renders API rows and wires only previous/next controls to server pagination", async () => {
    rpc.listAgents.mockImplementation(async ({ pageToken }) =>
      pageToken
        ? { agents: [grace], nextPageToken: "" }
        : { agents: [ada], nextPageToken: "server-cursor" },
    );
    render(<RegistryHarness />);
    await screen.findByRole("row", { name: "Edit Ada" });
    expect(
      (
        screen.getByRole("button", {
          name: "Previous page",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(screen.queryByRole("tab")).toBeNull();
    expect(screen.queryByRole("button", { name: /first page|last page/i })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Next page" }));
    await screen.findByRole("row", { name: "Edit Grace" });
    expect(screen.queryByRole("row", { name: "Edit Ada" })).toBeNull();
    expect(rpc.listAgents.mock.lastCall![0]).toEqual({
      pageToken: "server-cursor",
      pageSize: 10,
    });
    expect((screen.getByRole("button", { name: "Next page" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    fireEvent.click(screen.getByRole("button", { name: "Previous page" }));
    await screen.findByRole("row", { name: "Edit Ada" });
    expect(rpc.listAgents.mock.lastCall![0].pageToken).toBe("");
  });

  it("preserves the current table and offers retry if the next page fails", async () => {
    rpc.listAgents
      .mockResolvedValueOnce({ agents: [ada], nextPageToken: "server-cursor" })
      .mockRejectedValueOnce(new Error("Unavailable"))
      .mockResolvedValueOnce({ agents: [grace], nextPageToken: "" });
    render(<RegistryHarness />);
    await screen.findByRole("row", { name: "Edit Ada" });
    fireEvent.click(screen.getByRole("button", { name: "Next page" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("row", { name: "Edit Ada" })).toBeTruthy();
    expect(screen.getByText("Page 1")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await screen.findByRole("row", { name: "Edit Grace" });
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });
});

it("round-trips scoped capabilities through the registry editor", async () => {
  rpc.listAgents.mockResolvedValue({
    agents: [{ ...ada, capabilities: { workRead: BinaryPermission.YES } }],
    nextPageToken: "",
  });
  rpc.updateAgent.mockImplementation(async (patch) => ({ agent: { ...ada, ...patch } }));
  render(<RegistryHarness />);
  fireEvent.click(await screen.findByRole("row", { name: "Edit Ada" }));
  fireEvent.click(
    within(await screen.findByRole("tablist", { name: "Invoke agents in this thread" })).getByRole(
      "tab",
      { name: "Selected" },
    ),
  );
  await screen.findByRole("status", { name: "Invoke agents in this thread saved" });
  const target = "45e6cc6d-1581-4bb8-9f4b-8ac1e287143b";
  rpc.listAgents.mockResolvedValue({ agents: [{ ...grace, id: target }], nextPageToken: "" });
  fireEvent.click(
    screen.getByRole("button", { name: "Select agents for Invoke agents in this thread" }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "Grace" }));
  expect(rpc.updateAgent).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() =>
    expect(rpc.updateAgent).toHaveBeenCalledWith(
      expect.objectContaining({
        capabilities: expect.objectContaining({
          workRead: BinaryPermission.YES,
          agentsInvoke: expect.objectContaining({ mode: TargetSelection.SELECTED, ids: [target] }),
        }),
      }),
    ),
  );
});

it("pauses from the row action, keeps editing separate, and offers resume and delete only while paused", async () => {
  let paused = false;
  rpc.listAgents.mockImplementation(async () => ({
    agents: [{ ...ada, paused }],
    nextPageToken: "",
  }));
  rpc.pauseAgent.mockImplementation(async () => {
    paused = true;
    return {};
  });
  rpc.resumeAgent.mockImplementation(async () => {
    paused = false;
    return {};
  });
  render(<RegistryHarness />);
  const row = await screen.findByRole("row", { name: "Edit Ada" });
  expect(row.className).toContain("cursor-pointer");
  expect(screen.queryByRole("button", { name: "Edit Ada" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Delete Ada" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Pause Ada" }));
  await screen.findByRole("button", { name: "Resume Ada" });
  expect(rpc.pauseAgent).toHaveBeenCalledWith({ id: ada.id });
  expect(screen.queryByRole("region", { name: "Edit agent" })).toBeNull();
  expect(screen.getByText("Paused")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Delete Ada" }));
  await screen.findByRole("alertdialog");
  expect(rpc.deleteAgent).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  fireEvent.click(screen.getByRole("button", { name: "Resume Ada" }));
  await screen.findByRole("button", { name: "Pause Ada" });
  expect(rpc.resumeAgent).toHaveBeenCalledWith({ id: ada.id });
  expect(screen.queryByRole("button", { name: "Delete Ada" })).toBeNull();
  fireEvent.keyDown(screen.getByRole("row", { name: "Edit Ada" }), { key: "Enter" });
  await screen.findByRole("heading", { name: "Ada" });
});

it("reports asynchronous cancellation when paused", async () => {
  rpc.listAgents
    .mockResolvedValueOnce({ agents: [ada], nextPageToken: "" })
    .mockResolvedValue({ agents: [{ ...ada, paused: true }], nextPageToken: "" });
  rpc.pauseAgent.mockResolvedValue({});
  render(<RegistryHarness />);
  fireEvent.click(await screen.findByRole("button", { name: "Pause Ada" }));
  await screen.findByText(/Cancellation requested for running invocations/);
  expect(screen.queryByRole("button", { name: "Retry stop" })).toBeNull();
  expect(rpc.pauseAgent).toHaveBeenCalledTimes(1);
});
