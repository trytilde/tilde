import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
import {
  LogRecordSchema,
  LogMetricBucketSchema,
  LogsState,
} from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
const rpc = vi.hoisted(() => ({
  getLogsStatus: vi.fn(),
  listLogs: vi.fn(),
  getLogMetrics: vi.fn(),
}));
vi.mock("@/client", () => ({ logs: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});
beforeEach(() => {
  rpc.getLogMetrics.mockResolvedValue({ buckets: [] });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private callback: ResizeObserverCallback) {}
      observe(target: Element) {
        this.callback(
          [
            {
              target,
              contentRect: new DOMRect(0, 0, 1000, 600),
              borderBoxSize: [{ blockSize: 600, inlineSize: 1000 }],
              contentBoxSize: [{ blockSize: 600, inlineSize: 1000 }],
              devicePixelContentBoxSize: [],
            },
          ],
          this,
        );
      }
      unobserve() {}
      disconnect() {}
    },
  );
  HTMLElement.prototype.scrollTo = vi.fn();
  HTMLElement.prototype.scrollIntoView = vi.fn();
  vi.stubGlobal("scrollTo", vi.fn());
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi
      .fn()
      .mockReturnValue({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }),
  });
});
function mount(initial = "/agent/agent-one/logs") {
  const root = createRootRoute(),
    route = createRoute({
      getParentRoute: () => root,
      path: "/agent/$agentId/logs",
      validateSearch: parseLogSearch,
      component: () => <AgentLogs agentId="agent-one" />,
    });
  const router = createRouter({
    routeTree: root.addChildren([route]),
    history: createMemoryHistory({ initialEntries: [initial] }),
  });
  render(<RouterProvider router={router} />);
  return router;
}
const record = create(LogRecordSchema, {
  id: "log-one",
  timestamp: "2026-09-12T08:00:00Z",
  severity: "ERROR",
  severityNumber: 17,
  body: "<script>unsafe()</script>",
  invocationId: "00000000-0000-4000-8000-000000000001",
  traceId: "a".repeat(32),
  service: "example-agent-1",
  attributes: [{ key: "answer", value: "42" }],
});
function ready() {
  rpc.getLogsStatus.mockResolvedValue({ state: LogsState.READY });
  rpc.listLogs.mockResolvedValue({ records: [record], nextCursor: "" });
}
it("disables stored-log controls without issuing queries", async () => {
  rpc.getLogsStatus.mockResolvedValue({
    state: LogsState.DISABLED,
    message: "Log storage disabled",
  });
  mount();
  await screen.findByText("Log storage disabled");
  expect(rpc.listLogs).not.toHaveBeenCalled();
  expect(rpc.getLogMetrics).not.toHaveBeenCalled();
  expect((screen.getByRole("combobox", { name: "Search logs" }) as HTMLInputElement).disabled).toBe(
    true,
  );
});
it("uses Time/Level/Messages columns, filter chips, scroll pagination, and a safe right-side inspector", async () => {
  ready();
  rpc.listLogs.mockImplementation((r) =>
    Promise.resolve({
      records: r.cursor
        ? [record, create(LogRecordSchema, { ...record, id: "log-two" })]
        : [record],
      nextCursor: r.cursor ? "" : "page-two",
    }),
  );
  const router = mount();
  const first = await screen.findByRole("row", { name: "Inspect log log-one" });
  expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual([
    "Time",
    "Level",
    "Messages",
  ]);
  expect(within(first).getAllByRole("cell")[0].textContent).toMatch(
    /^12\/09\/26 \d{2}:\d{2}:\d{2}$/,
  );
  fireEvent.click(screen.getByRole("tab", { name: "Errors" }));
  await screen.findByRole("button", { name: "Edit severity:ERROR" });
  const input = screen.getByRole("combobox", { name: "Search logs" });
  fireEvent.change(input, { target: { value: 'message:"failure"' } });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() =>
    expect(router.state.location.search).toMatchObject({ text: "failure", severity: "17" }),
  );
  await waitFor(() =>
    expect(rpc.listLogs.mock.lastCall?.[0].filter).toMatchObject({
      search: "failure",
      minimumSeverity: 17,
    }),
  );
  expect(screen.getByRole("button", { name: "Edit message:failure" })).toBeTruthy();
  fireEvent.wheel(screen.getByRole("region", { name: "Log records scroll area" }), { deltaY: 120 });
  await screen.findByRole("row", { name: "Inspect log log-two" });
  expect(screen.getAllByRole("row", { name: "Inspect log log-one" }).length).toBe(1);
  expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  fireEvent.click(screen.getByRole("row", { name: "Inspect log log-one" }));
  const sheet = await screen.findByRole("dialog");
  expect(sheet.getAttribute("data-side")).toBe("right");
  expect(within(sheet).getByRole("link", { name: "View trace" }).getAttribute("href")).toContain(
    `trace=${"a".repeat(32)}`,
  );
  expect(document.querySelector("script")).toBeNull();
  expect(within(sheet).getByText(record.body).tagName).toBe("PRE");
});
it("shows whole-range log counts without latency and preserves the histogram when brushing", async () => {
  ready();
  rpc.getLogMetrics.mockResolvedValue({
    buckets: [0, 250, 0, 100].map((count, index) =>
      create(LogMetricBucketSchema, {
        startTime: `2026-09-12T0${index}:00:00Z`,
        endTime: `2026-09-12T0${index + 1}:00:00Z`,
        logCount: BigInt(count),
        errorCount: index === 1 ? 25n : 0n,
      }),
    ),
  });
  const router = mount(
    "/agent/agent-one/logs?range=custom&from=2026-09-12T00:00:00Z&to=2026-09-12T04:00:00Z&service=example-agent-1",
  );
  const region = await screen.findByRole("region", { name: "Log activity" });
  await waitFor(() => expect(region.querySelectorAll('[data-slot="activity-bar"]').length).toBe(4));
  const bars = region.querySelectorAll('[data-slot="activity-bar"]');
  expect(bars[1].getAttribute("data-errors")).toBe("true");
  expect(bars[3].getAttribute("data-errors")).toBe("false");
  expect(region.querySelector('[data-slot="activity-latency"]')).toBeNull();
  expect(within(region).queryByText(/Avg latency/)).toBeNull();
  const chart = within(region).getByRole("group", { name: "Log count over time" });
  const calls = rpc.getLogMetrics.mock.calls.length;
  for (const [type, x] of [
    ["pointerdown", 700],
    ["pointermove", 300],
    ["pointerup", 300],
  ] as const)
    fireEvent(chart, new MouseEvent(type, { bubbles: true, button: 0, clientX: x }));
  await waitFor(() =>
    expect(Date.parse(router.state.location.search.from as string)).toBeGreaterThan(
      Date.parse("2026-09-12T00:00:00Z"),
    ),
  );
  expect(rpc.getLogMetrics.mock.calls.length).toBe(calls);
  expect(region.querySelectorAll('[data-slot="activity-bar"]')[1]).toBe(bars[1]);
  expect(within(region).getAllByRole("slider").length).toBe(2);
  await waitFor(() =>
    expect(rpc.listLogs.mock.lastCall?.[0].filter.fromTime).toBe(router.state.location.search.from),
  );
  fireEvent.click(screen.getByRole("button", { name: "Clear date range" }));
  await waitFor(() => expect(router.state.location.search.range).toBeUndefined());
  expect(router.state.location.search.service).toBe("example-agent-1");
  expect(screen.getByRole("combobox", { name: "Log time range" }).textContent).toContain(
    "Last 24 hours",
  );
});
it("aborts old log pages when filters change", async () => {
  ready();
  let resolveOld!: (value: unknown) => void;
  let oldSignal: AbortSignal | undefined;
  rpc.listLogs.mockImplementation((r, options) =>
    r.cursor
      ? ((oldSignal = options.signal),
        new Promise((resolve) => {
          resolveOld = resolve;
        }))
      : Promise.resolve({
          records: [record],
          nextCursor: r.filter.minimumSeverity ? "" : "page-two",
        }),
  );
  mount();
  await screen.findByRole("row", { name: "Inspect log log-one" });
  fireEvent.wheel(screen.getByRole("region", { name: "Log records scroll area" }), { deltaY: 120 });
  await waitFor(() => expect(resolveOld).toBeTypeOf("function"));
  fireEvent.click(screen.getByRole("tab", { name: "Errors" }));
  await waitFor(() => expect(oldSignal?.aborted).toBe(true));
  await act(async () =>
    resolveOld({ records: [create(LogRecordSchema, { ...record, id: "stale" })], nextCursor: "" }),
  );
  expect(screen.queryByRole("row", { name: "Inspect log stale" })).toBeNull();
});
