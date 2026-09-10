import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AgentHealthStatus } from "@/gen/tilde/types/v1/agent_pb.js";
import { AgentRegistry } from "./agent-registry";

const rpc = vi.hoisted(() => ({
  listAgents: vi.fn(),
  createAgent: vi.fn(),
  updateAgent: vi.fn(),
  deleteAgent: vi.fn(),
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
    render(<AgentRegistry />);
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
    render(<AgentRegistry />);
    await screen.findByRole("button", { name: "Ada" });
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
    await screen.findByRole("button", { name: "Grace" });
    expect(screen.queryByRole("button", { name: "Ada" })).toBeNull();
    expect(rpc.listAgents.mock.lastCall![0]).toEqual({
      pageToken: "server-cursor",
      pageSize: 10,
    });
    expect((screen.getByRole("button", { name: "Next page" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    fireEvent.click(screen.getByRole("button", { name: "Previous page" }));
    await screen.findByRole("button", { name: "Ada" });
    expect(rpc.listAgents.mock.lastCall![0].pageToken).toBe("");
  });

  it("preserves the current table and offers retry if the next page fails", async () => {
    rpc.listAgents
      .mockResolvedValueOnce({ agents: [ada], nextPageToken: "server-cursor" })
      .mockRejectedValueOnce(new Error("Unavailable"))
      .mockResolvedValueOnce({ agents: [grace], nextPageToken: "" });
    render(<AgentRegistry />);
    await screen.findByRole("button", { name: "Ada" });
    fireEvent.click(screen.getByRole("button", { name: "Next page" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("button", { name: "Ada" })).toBeTruthy();
    expect(screen.getByText("Page 1")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await screen.findByRole("button", { name: "Grace" });
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });
});

it("round-trips scoped capabilities through the registry editor", async () => {
  rpc.listAgents.mockResolvedValue({
    agents: [{ ...ada, capabilities: { grants: { "work.read": { mode: "any", ids: [] } } } }],
    nextPageToken: "",
  });
  rpc.updateAgent.mockResolvedValue({ agent: ada });
  render(<AgentRegistry />);
  fireEvent.click(await screen.findByRole("button", { name: "Ada" }));
  fireEvent.change(await screen.findByLabelText("Invoke agents in this thread"), {
    target: { value: "only" },
  });
  const target = "45e6cc6d-1581-4bb8-9f4b-8ac1e287143b";
  fireEvent.change(screen.getByLabelText("Invoke agents in this thread targets"), {
    target: { value: target },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() =>
    expect(rpc.updateAgent).toHaveBeenCalledWith(
      expect.objectContaining({
        capabilities: {
          grants: {
            "work.read": { mode: "any", ids: [] },
            "agents.invoke": { mode: "only", ids: [target] },
          },
        },
      }),
    ),
  );
});
