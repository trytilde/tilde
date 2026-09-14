import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { create } from "@bufbuild/protobuf";
import { AgentLogs } from "./agent-logs";
import { parseLogSearch } from "./search";
import { LogRecordSchema, LogsState } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
const rpc = vi.hoisted(() => ({ getLogsStatus: vi.fn(), listLogs: vi.fn() }));
vi.mock("@/client", () => ({ logs: rpc }));
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
    path: "/agent/$agentId/logs",
    validateSearch: parseLogSearch,
    component: () => <AgentLogs agentId="agent-one" />,
  });
  const router = createRouter({
    routeTree: root.addChildren([route]),
    history: createMemoryHistory({ initialEntries: ["/agent/agent-one/logs"] }),
  });
  render(<RouterProvider router={router} />);
  return router;
}
it("disables stored-log controls without issuing queries", async () => {
  rpc.getLogsStatus.mockResolvedValue({
    state: LogsState.DISABLED,
    message: "Log storage disabled",
  });
  mount();
  await screen.findByText("Log storage disabled");
  expect(rpc.listLogs).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Apply" }).closest("fieldset")?.disabled).toBe(true);
});
it("keeps filters in the URL, loads pages, and safely inspects correlated logs", async () => {
  rpc.getLogsStatus.mockResolvedValue({ state: LogsState.READY });
  const record = create(LogRecordSchema, {
    id: "log-one",
    timestamp: "2026-09-12T08:00:00Z",
    severity: "ERROR",
    body: "<script>unsafe()</script>",
    invocationId: "invocation-one",
    traceId: "trace-one",
    attributes: [{ key: "answer", value: "42" }],
  });
  rpc.listLogs.mockImplementation((r) =>
    Promise.resolve({
      records: r.cursor ? [create(LogRecordSchema, { ...record, id: "log-two" })] : [record],
      nextCursor: r.cursor ? "" : "page-two",
    }),
  );
  const router = mount();
  await screen.findByRole("row", { name: "Inspect log log-one" });
  fireEvent.change(screen.getByLabelText("Search message"), { target: { value: "failure" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  await waitFor(() => expect(router.state.location.search).toMatchObject({ text: "failure" }));
  await waitFor(() =>
    expect(rpc.listLogs).toHaveBeenLastCalledWith(
      expect.objectContaining({ filter: expect.objectContaining({ search: "failure" }) }),
      expect.anything(),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "Load more" }));
  await screen.findByRole("row", { name: "Inspect log log-two" });
  fireEvent.click(screen.getByRole("row", { name: "Inspect log log-one" }));
  await screen.findByRole("dialog");
  expect(screen.getByRole("link", { name: "View trace" }).getAttribute("href")).toContain(
    "trace=trace-one",
  );
  expect(document.querySelector("script")).toBeNull();
});
