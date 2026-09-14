import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  createRootRoute,
  createRoute,
  createRouter,
  createMemoryHistory,
  RouterProvider,
} from "@tanstack/react-router";
import { create } from "@bufbuild/protobuf";
import { AgentTracing } from "./agent-tracing";
import { parseSearch } from "./search";
import {
  ObservationSchema,
  TracingState,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
const rpc = vi.hoisted(() => ({
  getTracingStatus: vi.fn(),
  listObservations: vi.fn(),
  getTrace: vi.fn(),
  getSession: vi.fn(),
}));
vi.mock("@/client", () => ({ traces: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  localStorage.clear();
});
beforeEach(() => {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi
      .fn()
      .mockReturnValue({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }),
  });
});
function mount() {
  const root = createRootRoute();
  const route = createRoute({
    getParentRoute: () => root,
    path: "/tracing",
    validateSearch: parseSearch,
    component: () => <AgentTracing agentId="agent-one" />,
  });
  const router = createRouter({
    routeTree: root.addChildren([route]),
    history: createMemoryHistory({ initialEntries: ["/tracing"] }),
  });
  render(<RouterProvider router={router} />);
  return router;
}
const observation = create(ObservationSchema, {
  id: "span-1",
  traceId: "trace-1",
  sessionId: "session-1",
  name: "Recipe model",
  type: "GENERATION",
  level: "DEFAULT",
  startTime: "2026-09-11T10:00:00Z",
  endTime: "2026-09-11T10:00:01Z",
  input: '{"prompt":"lasagna"}',
  output: "A recipe",
  model: "gpt-4o-mini",
  totalTokens: 12,
  costUsd: 0.001,
  latencySeconds: 1,
  traceUrl: "https://langfuse.example/project/p/traces/trace-1",
  observationUrl: "https://langfuse.example/project/p/traces/trace-1?observation=span-1",
  sessionUrl: "https://langfuse.example/project/p/sessions/session-1",
});
describe("Langfuse agent viewer", () => {
  it("renders the disabled state without fetching observations", async () => {
    rpc.getTracingStatus.mockResolvedValue({
      state: TracingState.DISABLED,
      message: "Tracing is not configured for this installation",
      projectUrl: "",
    });
    mount();
    await screen.findByText("Tracing is not configured for this installation");
    expect(screen.getByRole("button", { name: "Filters" }).closest("fieldset")?.disabled).toBe(
      true,
    );
    expect(rpc.listObservations).not.toHaveBeenCalled();
    expect(screen.queryByRole("link", { name: "Open project in Langfuse" })).toBeNull();
  });
  it("filters through the URL, groups sessions, and opens an inspector with external links", async () => {
    rpc.getTracingStatus.mockResolvedValue({
      state: TracingState.READY,
      projectUrl: "https://langfuse.example/project/p",
    });
    rpc.listObservations.mockResolvedValue({
      observations: [observation],
      nextCursor: "next",
      partial: true,
    });
    rpc.getTrace.mockResolvedValue({ observations: [observation], nextCursor: "", partial: false });
    rpc.getSession.mockResolvedValue({
      observations: [observation],
      nextCursor: "",
      partial: false,
    });
    const router = mount();
    await screen.findByRole("row", { name: "Inspect Recipe model" });
    const link = screen.getByRole("link", { name: "Open project in Langfuse" });
    expect(link.getAttribute("target")).toBe("_blank");
    expect(link.getAttribute("rel")).toBe("noopener noreferrer");
    fireEvent.click(screen.getByRole("button", { name: "LLM Calls" }));
    await waitFor(() => expect(router.state.location.search.preset).toBe("llm"));
    await waitFor(() =>
      expect(rpc.listObservations.mock.lastCall?.[0].filter.type).toBe("GENERATION"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Filters" }));
    fireEvent.change(screen.getByLabelText("Input search"), { target: { value: "lasagna" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(rpc.listObservations.mock.lastCall?.[0].filter.inputSearch).toBe("lasagna"),
    );
    fireEvent.click(await screen.findByRole("row", { name: "Inspect Recipe model" }));
    await screen.findByRole("dialog");
    expect(
      screen.getByRole("link", { name: "Open observation in Langfuse" }).getAttribute("href"),
    ).toContain("observation=span-1");
    expect(within(screen.getByRole("dialog")).getByText("A recipe")).toBeTruthy();
  });
});
