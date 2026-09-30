import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
  TraceSessionSummarySchema,
  ObservationMetricBucketSchema,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
/** Whether a server filter carries the condition. */
function has(
  filter: { conditions?: { column: string; operator: string; values: string[] }[] } | undefined,
  column: string,
  operator: string,
  value: string,
) {
  return (filter?.conditions ?? []).some(
    (c) => c.column === column && c.operator === operator && c.values.includes(value),
  );
}
const rpc = vi.hoisted(() => ({
  getObservationMetrics: vi.fn(),
  listObservations: vi.fn(),
  getTrace: vi.fn(),
  getSession: vi.fn(),
  getTraceObjectUrl: vi.fn(),
}));
vi.mock("@/client", () => ({ traces: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});
beforeEach(() => {
  rpc.getObservationMetrics.mockResolvedValue({ buckets: [] });
  vi.stubGlobal(
    "ResizeObserver",
    class implements ResizeObserver {
      constructor(private callback: ResizeObserverCallback) {}
      observe(target: Element) {
        const size = [{ blockSize: 600, inlineSize: 1000 }];
        this.callback(
          [
            {
              target,
              contentRect: new DOMRect(0, 0, 1000, 600),
              borderBoxSize: size,
              contentBoxSize: size,
              devicePixelContentBoxSize: size,
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
function mount(initial = "/tracing") {
  const root = createRootRoute();
  const route = createRoute({
    getParentRoute: () => root,
    path: "/tracing",
    validateSearch: parseSearch,
    component: () => <AgentTracing agentId="agent-one" />,
  });
  const router = createRouter({
    routeTree: root.addChildren([
      route,
      createRoute({
        getParentRoute: () => root,
        path: "/sessions",
        validateSearch: parseSearch,
        component: () => <AgentTracing agentId="agent-one" view="sessions" />,
      }),
    ]),
    history: createMemoryHistory({ initialEntries: [initial] }),
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
  input: '{"messages":[{"role":"user","content":"Make a recipe"}]}',
  output: "**A recipe**",
  model: "gpt-4o-mini",
  totalTokens: 12,
  costUsd: 0.001,
  latencySeconds: 1,
});
const second = create(ObservationSchema, {
  ...observation,
  id: "span-2",
  name: "Tool step",
  type: "TOOL",
  parentId: "span-1",
  startTime: "2026-09-11T10:00:00.2Z",
  endTime: "2026-09-11T10:00:00.7Z",
  latencySeconds: 0.5,
  output: "Tool result",
});
const page = (observations = [observation], nextCursor = "") => ({
  observations,
  sessions: [],
  nextCursor,
  partial: !!nextCursor,
});

it("renders a shared link whose where expression is malformed instead of failing", async () => {
  rpc.listObservations.mockResolvedValue(page());
  expect(parseSearch({ where: "level:" }).where).toBeUndefined();
  expect(parseSearch({ where: 'name:"open' }).where).toBeUndefined();
  expect(parseSearch({ where: "latency:>2 -level:DEBUG" }).where).toBe("latency:>2 -level:DEBUG");
  mount("/tracing?where=level%3A&model=gpt-4o-mini");
  await screen.findByRole("row", { name: "Inspect Recipe model" });
  expect(
    (screen.getByRole("combobox", { name: "Search traces" }) as HTMLInputElement).disabled,
  ).toBe(false);
  expect(has(rpc.listObservations.mock.lastCall?.[0].filter, "model", "=", "gpt-4o-mini")).toBe(
    true,
  );
  expect(rpc.listObservations.mock.lastCall?.[0].filter.conditions).toHaveLength(1);
});
it("resolves a cooperating agent's media under that agent's id, not the viewer's", async () => {
  rpc.listObservations.mockResolvedValue(page());
  const id = "a".repeat(64);
  const other = create(ObservationSchema, {
    ...second,
    agentId: "agent-two",
    output: JSON.stringify({
      role: "assistant",
      content: [
        {
          type: "text",
          text: `see @@@tildeMedia:type=image/png|id=${id}|source=base64_data_uri@@@`,
        },
      ],
    }),
  });
  rpc.getTrace.mockResolvedValue(page([{ ...observation, agentId: "agent-one" }, other]));
  rpc.getTraceObjectUrl.mockResolvedValue({ url: "https://objects.test/signed", expiresIn: 600 });
  mount();
  fireEvent.click(await screen.findByRole("row", { name: "Inspect Recipe model" }));
  const sheet = await screen.findByRole("dialog");
  await within(sheet).findByText("Make a recipe");
  const plot = await waitFor(() => {
    const plot = sheet.querySelector<HTMLElement>('[data-slot="waterfall-plot"]');
    expect(plot).not.toBeNull();
    return plot!;
  });
  const rowHeight = parseFloat(plot.style.height) / 2;
  fireEvent.click(plot, { detail: 1, clientX: 790, clientY: 1.5 * rowHeight });
  fireEvent.click(within(sheet).getByRole("tab", { name: "Output" }));
  await waitFor(() =>
    expect(rpc.getTraceObjectUrl).toHaveBeenCalledWith(
      { agentId: "agent-two", key: `media/agent-two/${id}` },
      expect.anything(),
    ),
  );
  await waitFor(() =>
    expect(sheet.querySelector('img[src="https://objects.test/signed"]')).not.toBeNull(),
  );
});
it("suggests fields and values, applies searches to the server, and preserves invalid drafts", async () => {
  rpc.listObservations.mockResolvedValue(page());
  const router = mount();
  await screen.findByRole("row", { name: "Inspect Recipe model" });
  const input = screen.getByRole("combobox", { name: "Search traces" });
  await act(async () => input.focus());
  fireEvent.click(input);
  fireEvent.change(input, { target: { value: "lev" } });
  fireEvent.select(input, { target: { selectionStart: 3, selectionEnd: 3 } });
  fireEvent.keyDown(input, { key: "ArrowDown" });
  fireEvent.click(await screen.findByRole("option", { name: /level:/ }));
  await waitFor(() => expect((input as HTMLInputElement).value).toBe("level:"));
  fireEvent.click(input);
  fireEvent.click(await screen.findByRole("option", { name: /^ERROR/ }));
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => expect(router.state.location.search.level).toBe("ERROR"));
  expect(has(rpc.listObservations.mock.lastCall?.[0].filter, "level", "=", "ERROR")).toBe(true);
  fireEvent.change(input, { target: { value: 'model:gpt-4o-mini input:"lasagna recipe"' } });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() =>
    expect(
      has(rpc.listObservations.mock.lastCall?.[0].filter, "input", "contains", "lasagna recipe"),
    ).toBe(true),
  );
  const calls = rpc.listObservations.mock.calls.length;
  fireEvent.change(input, { target: { value: 'input:"unfinished' } });
  fireEvent.submit(input.closest("form")!);
  await screen.findByRole("alert");
  expect(rpc.listObservations).toHaveBeenCalledTimes(calls);
  expect((input as HTMLInputElement).value).toBe('input:"unfinished');
  fireEvent.keyDown(input, { key: "Escape" });
  fireEvent.click(await screen.findByRole("button", { name: "Clear search" }));
  await waitFor(() => expect(router.state.location.search.input).toBeUndefined());
  fireEvent.change(input, { target: { value: 'name:"Recipe model" model:gpt-4o-mini' } });
  fireEvent.submit(input.closest("form")!);
  await screen.findByRole("button", { name: 'Edit name:"Recipe model"' });
  expect((input as HTMLInputElement).value).toBe("");
  fireEvent.click(screen.getByRole("button", { name: "Remove model:gpt-4o-mini" }));
  await waitFor(() => expect(router.state.location.search.model).toBeUndefined());
  expect(router.state.location.search.name).toBe("Recipe model");
  fireEvent.click(screen.getByRole("button", { name: 'Edit name:"Recipe model"' }));
  expect((input as HTMLInputElement).value).toBe('name:"Recipe model"');
  fireEvent.change(input, { target: { value: 'name:"Tool step"' } });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() =>
    expect(
      has(rpc.listObservations.mock.lastCall?.[0].filter, "name", "contains", "Tool step"),
    ).toBe(true),
  );
  // Filter operators travel in `where`: numbers, negation, any-of and span attributes.
  fireEvent.change(input, {
    target: { value: "latency:>2 -level:DEBUG type:GENERATION|TOOL gen_ai.tool.name:search" },
  });
  fireEvent.submit(input.closest("form")!);
  await waitFor(() =>
    expect(router.state.location.search.where).toBe(
      "latency:>2 -level:DEBUG type:GENERATION|TOOL gen_ai.tool.name:search",
    ),
  );
  // The earlier name chip stays applied alongside the new terms.
  const sent = rpc.listObservations.mock.lastCall?.[0].filter.conditions;
  expect(sent).toHaveLength(5);
  expect(sent).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ column: "latency", operator: ">", values: ["2"] }),
      expect.objectContaining({ column: "level", operator: "none of", values: ["DEBUG"] }),
      expect.objectContaining({
        column: "type",
        operator: "any of",
        values: ["GENERATION", "TOOL"],
      }),
      expect.objectContaining({ column: "metadata", key: "gen_ai.tool.name", operator: "=" }),
    ]),
  );
});
it("applies calendar and time changes immediately and keeps invalid ranges off the server", async () => {
  rpc.listObservations.mockResolvedValue(page());
  mount("/tracing?range=custom&from=2026-09-10T10:00:00Z&to=2026-09-11T10:00:00Z");
  await screen.findByRole("row", { name: "Inspect Recipe model" });
  const startDate = screen.getByRole("button", { name: /^Start date/ });
  expect(startDate.textContent).toBe("10/09/26");
  expect(screen.queryByRole("button", { name: "Apply range" })).toBeNull();
  fireEvent.click(startDate);
  fireEvent.click(await screen.findByRole("button", { name: /, 9 September 2026/ }));
  await waitFor(() =>
    expect(rpc.listObservations.mock.lastCall?.[0].filter.fromTime.slice(0, 10)).toBe("2026-09-09"),
  );
  fireEvent.change(screen.getByLabelText(/^Start time/), { target: { value: "10:30:15" } });
  await waitFor(() =>
    expect(rpc.listObservations.mock.lastCall?.[0].filter.fromTime).toBe(
      new Date("2026-09-09T10:30:15").toISOString(),
    ),
  );
  const calls = rpc.listObservations.mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: /^End date/ }));
  fireEvent.click(await screen.findByRole("button", { name: /, 8 September 2026/ }));
  await screen.findByRole("alert");
  expect(rpc.listObservations.mock.calls.length).toBe(calls);
  fireEvent.click(screen.getByRole("button", { name: /^End date/ }));
  fireEvent.click(await screen.findByRole("button", { name: /10 September 2026/ }));
  await waitFor(() =>
    expect(rpc.listObservations.mock.lastCall?.[0].filter.toTime.slice(0, 10)).toBe("2026-09-10"),
  );
});
it("loads cursor pages on scroll, preserves earlier rows, and resets on filter changes", async () => {
  rpc.listObservations.mockImplementation(async ({ cursor }) =>
    cursor ? page([second]) : page([observation], "page-two"),
  );
  const router = mount();
  await screen.findByRole("row", { name: "Inspect Recipe model" });
  expect(screen.queryByRole("button", { name: "Next page" })).toBeNull();
  const filter = rpc.listObservations.mock.calls[0][0].filter;
  fireEvent.wheel(screen.getByRole("region", { name: "Trace observations scroll area" }), {
    deltaY: 120,
  });
  await screen.findByRole("row", { name: "Inspect Tool step" });
  expect(screen.getByRole("row", { name: "Inspect Recipe model" })).toBeTruthy();
  expect(rpc.listObservations.mock.lastCall?.[0]).toMatchObject({ cursor: "page-two", filter });
  fireEvent.click(screen.getByRole("tab", { name: "Errors" }));
  await waitFor(() => expect(router.state.location.search.preset).toBe("errors"));
  await waitFor(() => expect(screen.queryByRole("row", { name: "Inspect Tool step" })).toBeNull());
  expect(rpc.listObservations.mock.lastCall?.[0].cursor).toBe("");
  expect(has(rpc.listObservations.mock.lastCall?.[0].filter, "level", "=", "ERROR")).toBe(true);
});
it("uses latest trace metadata for session facts while deduplicating paged trace metrics", async () => {
  const details = create(TraceSessionSummarySchema, {
    id: "session-1",
    providerId: "whatsapp",
    providerName: "WhatsApp",
    providerIconUrl: "https://example.com/whatsapp.svg",
    identities: ["+447700900123", "+447700900456"],
    messageCount: 7n,
    metadataTime: "2026-09-11T10:00:03Z",
    lastTurnTime: "2026-09-11T10:00:02Z",
  });
  rpc.listObservations.mockImplementation(async ({ cursor }) => ({
    ...page(cursor ? [observation, second] : [observation], cursor ? "" : "page-two"),
    sessions: [
      cursor
        ? create(TraceSessionSummarySchema, {
            ...details,
            messageCount: 12n,
            metadataTime: "2026-09-11T09:00:00Z",
          })
        : details,
    ],
  }));
  mount("/sessions");
  let row = await screen.findByRole("row", { name: "Inspect session session-1" });
  expect(within(row).getAllByRole("cell")[4].textContent).toBe("7");
  expect(within(row).getByText("WhatsApp")).toBeTruthy();
  expect(within(row).getByText("+447700900123, +447700900456")).toBeTruthy();
  expect(row.querySelector("img")?.getAttribute("src")).toBe(details.providerIconUrl);
  expect(screen.queryByRole("tablist", { name: "Tracing views" })).toBeNull();
  expect(
    within(screen.getByRole("table", { name: "Trace sessions" }))
      .getAllByRole("columnheader")
      .map((h) => h.textContent),
  ).toEqual([
    "Provider",
    "Session ID",
    "Identities",
    "Last turn",
    "Messages",
    "Errors",
    "Tokens",
    "Cost (USD)",
  ]);
  fireEvent.wheel(screen.getByRole("region", { name: "Trace sessions scroll area" }), {
    deltaY: 120,
  });
  await waitFor(() =>
    expect(
      within(screen.getByRole("row", { name: "Inspect session session-1" })).getAllByRole("cell")[6]
        .textContent,
    ).toBe("24"),
  );
  row = screen.getByRole("row", { name: "Inspect session session-1" });
  const cells = within(row).getAllByRole("cell");
  expect(cells[4].textContent).toBe("7");
  expect(cells[7].textContent).toBe("0.002000");
  expect(rpc.listObservations).toHaveBeenCalledTimes(2);
});

it("discards an old scrolling response when the query changes", async () => {
  let resolveOld!: (value: unknown) => void;
  let signal: AbortSignal | undefined;
  rpc.listObservations.mockImplementation(({ cursor, filter }, options) => {
    if (cursor) {
      signal = options.signal;
      return new Promise((resolve) => {
        resolveOld = resolve;
      });
    }
    return Promise.resolve(
      has(filter, "level", "=", "ERROR") ? page([second]) : page([observation], "page-two"),
    );
  });
  mount();
  await screen.findByRole("row", { name: "Inspect Recipe model" });
  fireEvent.wheel(screen.getByRole("region", { name: "Trace observations scroll area" }), {
    deltaY: 100,
  });
  await waitFor(() => expect(resolveOld).toBeTypeOf("function"));
  fireEvent.click(screen.getByRole("tab", { name: "Errors" }));
  await screen.findByRole("row", { name: "Inspect Tool step" });
  expect(signal?.aborted).toBe(true);
  await act(async () => resolveOld(page([observation])));
  expect(screen.queryByRole("row", { name: "Inspect Recipe model" })).toBeNull();
});
it("opens a right-side sheet with a selectable waterfall and formatted detail tabs", async () => {
  rpc.listObservations.mockResolvedValue(page());
  const extraSpans = Array.from({ length: 43 }, (_, index) =>
    create(ObservationSchema, {
      ...observation,
      id: `extra-${index}`,
      name: `Extra span ${index}`,
      parentId: observation.id,
      startTime: `2026-09-11T10:00:02.${String(index * 10).padStart(3, "0")}Z`,
      endTime: `2026-09-11T10:00:02.${String(index * 10 + 5).padStart(3, "0")}Z`,
    }),
  );
  rpc.getTrace.mockResolvedValue(page([observation, second, ...extraSpans]));
  mount();
  const row = await screen.findByRole("row", { name: "Inspect Recipe model" });
  expect(screen.getByRole("columnheader", { name: "Latency (s)" })).toBeTruthy();
  expect(within(row).getByText("1.000")).toBeTruthy();
  expect(screen.queryByRole("columnheader", { name: "Trace ID" })).toBeNull();
  expect(screen.queryByRole("columnheader", { name: "Session ID" })).toBeNull();
  fireEvent.click(row);
  const sheet = await screen.findByRole("dialog");
  expect(sheet.getAttribute("data-side")).toBe("right");
  await within(sheet).findByText("Make a recipe");
  fireEvent.click(within(sheet).getByRole("tab", { name: "Output" }));
  expect(within(sheet).getByText("A recipe").tagName).toBe("STRONG");
  const chart = await waitFor(() => {
    const chart = sheet.querySelector<HTMLElement>(".mc-trace-live");
    expect(chart).not.toBeNull();
    return chart!;
  });
  expect(sheet.querySelectorAll("rect[data-mc-ink]").length).toBe(45);
  expect(within(sheet).getByRole("toolbar", { name: "Waterfall view controls" })).toBeTruthy();
  expect(within(sheet).getByLabelText("Time in milliseconds").textContent).toContain("500 ms");
  const plot = sheet.querySelector<HTMLElement>('[data-slot="waterfall-plot"]')!;
  const rowHeight = parseFloat(plot.style.height) / 45;
  // Hover empty space at the far right of the second row, outside its short bar.
  const hover = (index: number) =>
    fireEvent(
      plot,
      new MouseEvent("pointermove", {
        bubbles: true,
        clientX: 790,
        clientY: (index + 0.5) * rowHeight,
      }),
    );
  const summary = sheet.querySelector('[data-slot="waterfall-summary"]')!;
  const summaryText = summary.textContent;
  const ruler = () => sheet.querySelector('[data-slot="span-duration"]')!;
  expect(ruler().textContent).toBe("1,000 ms");
  hover(1);
  await within(sheet).findByText("Tool result");
  expect(summary.textContent).toBe(summaryText);
  expect(ruler().textContent).toBe("500 ms");
  const labels = sheet.querySelectorAll<SVGTextElement>(".trace-fold-chunk text");
  expect(labels[1].style.fontWeight).toBe("700");
  fireEvent.pointerLeave(plot);
  expect(labels[1].style.fontWeight).toBe("400");
  expect(ruler().textContent).toBe("1,000 ms");
  expect(within(sheet).getByText("A recipe").tagName).toBe("STRONG");
  hover(1);
  fireEvent.click(plot, { detail: 1, clientX: 790, clientY: 1.5 * rowHeight });
  fireEvent.pointerLeave(plot);
  expect(within(sheet).getByText("Tool result")).toBeTruthy();
  expect(ruler().textContent).toBe("500 ms");
  hover(0);
  expect(within(sheet).getByText("A recipe").tagName).toBe("STRONG");
  fireEvent.pointerLeave(plot);
  expect(within(sheet).getByText("Tool result")).toBeTruthy();
  fireEvent.keyDown(chart, { key: "Home" });
  fireEvent.keyDown(chart, { key: "Enter" });
  fireEvent.blur(chart);
  expect(within(sheet).getByText("A recipe").tagName).toBe("STRONG");
  // Zoom changes the time domain, preserving row geometry and vertical reading position.
  const navigation = sheet.querySelector<HTMLElement>(".waterfall-scroll")!;
  const fitWidth = 800;
  const grid = plot.querySelector("pattern")!;
  const fittedTickWidth = Number(grid.getAttribute("width"));

  Object.defineProperties(navigation, {
    clientWidth: { configurable: true, value: fitWidth },
    clientHeight: { configurable: true, value: 336 },
  });
  navigation.getBoundingClientRect = () => new DOMRect(0, 0, fitWidth, 336);
  const bar = sheet.querySelector<SVGRectElement>("rect[data-mc-ink]")!;
  const barHeight = bar.getAttribute("height");
  const barWidth = Number(bar.getAttribute("width"));
  const zoom = () => Number(grid.getAttribute("width")) / fittedTickWidth;
  navigation.scrollTop = 156;
  fireEvent.scroll(navigation);
  fireEvent.wheel(navigation, { ctrlKey: true, deltaY: -80, clientX: 400 });
  expect(zoom()).toBeGreaterThan(1);
  expect(bar.getAttribute("height")).toBe(barHeight);
  expect(Number(bar.getAttribute("width"))).toBeGreaterThan(barWidth);
  expect(plot.style.height).toBe(`${45 * 26}px`);
  expect(labels[0].getAttribute("font-size")).toBe("11");
  expect(navigation.scrollTop).toBe(156);
  const pinchedScale = zoom();
  const firstLabelX = labels[0].getAttribute("x");
  const wheel = new WheelEvent("wheel", {
    bubbles: true,
    cancelable: true,
    deltaX: 40,
    deltaY: 20,
  });
  fireEvent(navigation, wheel);
  expect(wheel.defaultPrevented).toBe(false); // Ordinary two-finger pan stays native.
  navigation.scrollLeft += 40;
  navigation.scrollTop += 20;
  fireEvent.scroll(navigation);
  expect(zoom()).toBe(pinchedScale);
  expect(labels[0].getAttribute("x")).toBe(firstLabelX);
  for (const [type, scale] of [
    ["gesturestart", 1],
    ["gesturechange", 1.5],
    ["gestureend", 1.5],
  ] as const) {
    fireEvent(
      navigation,
      Object.assign(new Event(type, { bubbles: true, cancelable: true }), { scale, clientX: 400 }),
    );
  }
  expect(zoom()).toBeCloseTo(pinchedScale * 1.5);
  expect(navigation.scrollTop).toBe(176);
  expect(bar.getAttribute("height")).toBe(barHeight);
  const chartSvg = sheet.querySelector<SVGSVGElement>(".mc-root.mc-trace")!;
  expect(chartSvg.style.height).toBe(`${40 * 26 + 2}px`);
  expect(chartSvg.getAttribute("preserveAspectRatio")).toBe("none");
  const lastLabel = labels[labels.length - 1];
  const lastChunk = lastLabel.closest<HTMLElement>(".trace-fold-chunk")!;
  expect(
    parseFloat(plot.style.width) -
      parseFloat(lastChunk.style.left) -
      Number(lastLabel.getAttribute("x")),
  ).toBeGreaterThan(lastLabel.textContent!.length * 7);
  fireEvent.click(within(sheet).getByRole("button", { name: "Fit timeline" }));
  expect(zoom()).toBe(1);
  expect(navigation.scrollLeft).toBe(0);
  expect(navigation.scrollTop).toBe(176);
  // A dragged row must not replace the clicked selection when the pointer is released.
  fireEvent(
    plot,
    new MouseEvent("pointerdown", {
      bubbles: true,
      buttons: 1,
      clientX: 300,
      clientY: rowHeight / 2,
    }),
  );
  fireEvent(
    plot,
    new MouseEvent("pointermove", {
      bubbles: true,
      buttons: 1,
      clientX: 350,
      clientY: rowHeight * 1.5,
    }),
  );
  fireEvent.click(plot, { detail: 1, clientX: 350, clientY: rowHeight * 1.5 });
  fireEvent.pointerLeave(plot);
  expect(within(sheet).getByText("A recipe").tagName).toBe("STRONG");
  expect(sheet.querySelectorAll("rect[data-mc-ink]").length).toBe(45);
  fireEvent.click(within(sheet).getByRole("button", { name: "Full screen" }));
  expect(
    within(sheet).getByRole("button", { name: "Exit full screen" }).getAttribute("aria-pressed"),
  ).toBe("true");
  expect(
    sheet.querySelector('[data-slot="trace-inspector-layout"]')!.classList.contains("grid-cols-2"),
  ).toBe(true);
  expect(within(sheet).getByText("A recipe").tagName).toBe("STRONG");
  fireEvent.click(within(sheet).getByRole("button", { name: "Exit full screen" }));
  expect(
    sheet.querySelector('[data-slot="trace-inspector-layout"]')!.classList.contains("flex-col"),
  ).toBe(true);
  fireEvent.click(within(sheet).getByRole("tab", { name: "Metadata" }));
  expect(
    within(within(sheet).getByRole("tabpanel", { name: "Metadata" })).getByText("trace-1"),
  ).toBeTruthy();
  expect(within(sheet).queryByText(/TTFT/)).toBeNull();
  // Traces are Tilde's own; nothing links out to another product.
  expect(within(sheet).queryByRole("link")).toBeNull();
});

it("charts full-range counts and latency, marks error buckets, and brushes the same server filters", async () => {
  rpc.listObservations.mockResolvedValue(page());
  const buckets = [200, 100, 0, 20].map((count, index) =>
    create(ObservationMetricBucketSchema, {
      startTime: `2026-09-11T0${index}:00:00Z`,
      endTime: `2026-09-11T0${index + 1}:00:00Z`,
      observationCount: BigInt(count),
      errorCount: BigInt(index === 0 ? 3 : 0),
      averageLatencySeconds: index === 2 ? undefined : index + 0.5,
    }),
  );
  rpc.getObservationMetrics.mockResolvedValue({ buckets });
  const router = mount(
    "/tracing?range=custom&from=2026-09-11T00:00:00Z&to=2026-09-11T04:00:00Z&name=model&model=gpt-4o-mini",
  );
  const region = await screen.findByRole("region", { name: "Trace activity" });
  await waitFor(() => expect(region.querySelectorAll('[data-slot="activity-bar"]').length).toBe(4));
  const bars = region.querySelectorAll('[data-slot="activity-bar"]');
  expect(bars.length).toBe(4);
  expect(bars[0].getAttribute("data-errors")).toBe("true");
  expect(bars[1].getAttribute("data-errors")).toBe("false");
  expect(
    region.querySelector('[data-slot="activity-latency"]')!.getAttribute("d")!.match(/L/g)?.length,
  ).toBe(2);
  const chart = within(region).getByRole("group", {
    name: "Observation count and average latency over time",
  });
  // A reversed drag behaves like a forward drag and commits only on pointer release.
  const calls = rpc.getObservationMetrics.mock.calls.length;
  fireEvent(chart, new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 700 }));
  fireEvent(chart, new MouseEvent("pointermove", { bubbles: true, clientX: 300 }));
  expect(region.querySelector('[data-slot="activity-selection"]')).toBeTruthy();
  expect(rpc.getObservationMetrics.mock.calls.length).toBe(calls);
  fireEvent(chart, new MouseEvent("pointerup", { bubbles: true, clientX: 300 }));
  const from = new Date(
    Date.parse("2026-09-11T00:00:00Z") + Math.round(((300 - 42) / (1000 - 55 - 42)) * 4 * 3600_000),
  ).toISOString();
  const to = new Date(
    Date.parse("2026-09-11T00:00:00Z") + Math.round(((700 - 42) / (1000 - 55 - 42)) * 4 * 3600_000),
  ).toISOString();
  await waitFor(() => expect(router.state.location.search.from).toBe(from));
  expect(router.state.location.search.to).toBe(to);
  expect(router.state.location.search.model).toBe("gpt-4o-mini");
  await waitFor(() =>
    expect(rpc.listObservations.mock.lastCall?.[0].filter).toMatchObject({
      fromTime: from,
      toTime: to,
      conditions: [
        expect.objectContaining({ column: "name", operator: "contains", values: ["model"] }),
        expect.objectContaining({ column: "model", operator: "=", values: ["gpt-4o-mini"] }),
      ],
    }),
  );
  // The chart keeps its data and axis while the table applies the highlighted range.
  expect(rpc.getObservationMetrics.mock.calls.length).toBe(calls);
  expect(rpc.getObservationMetrics.mock.lastCall?.[0].filter).toMatchObject({
    fromTime: "2026-09-11T00:00:00.000Z",
    toTime: "2026-09-11T04:00:00.000Z",
  });
  expect(region.querySelectorAll('[data-slot="activity-bar"]')[0]).toBe(bars[0]);
  expect(region.querySelector('[data-slot="activity-selection"]')).toBeTruthy();
  const originalMask = region
    .querySelector('[data-slot="activity-selection"]')!
    .getAttribute("width");
  const leftHandle = within(region).getByRole("slider", { name: "Selected range start" });
  fireEvent(leftHandle, new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 300 }));
  fireEvent(chart, new MouseEvent("pointermove", { bubbles: true, clientX: 200 }));
  fireEvent.keyDown(chart, { key: "Escape" });
  fireEvent(chart, new MouseEvent("pointerup", { bubbles: true, clientX: 200 }));
  expect(router.state.location.search.from).toBe(from);
  expect(region.querySelector('[data-slot="activity-selection"]')!.getAttribute("width")).toBe(
    originalMask,
  );
  const resizeEdge = (name: string, start: number, end: number) => {
    const handle = within(region).getByRole("slider", { name });
    fireEvent(handle, new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: start }));
    fireEvent(chart, new MouseEvent("pointermove", { bubbles: true, clientX: end }));
    fireEvent(chart, new MouseEvent("pointerup", { bubbles: true, clientX: end }));
  };
  resizeEdge("Selected range start", 300, 42);
  await waitFor(() => expect(router.state.location.search.from).toBe("2026-09-11T00:00:00.000Z"));
  expect(router.state.location.search.to).toBe(to);
  resizeEdge("Selected range end", 700, 945);
  await waitFor(() => expect(router.state.location.search.to).toBe("2026-09-11T04:00:00Z"));
  expect(region.querySelector('[data-slot="activity-selection"]')).toBeNull();
  expect(rpc.getObservationMetrics.mock.calls.length).toBe(calls);
  expect(router.state.location.search.model).toBe("gpt-4o-mini");
  fireEvent.click(screen.getByRole("button", { name: "Clear date range" }));
  await waitFor(() => expect(router.state.location.search.range).toBeUndefined());
  expect(router.state.location.search.from).toBeUndefined();
  expect(router.state.location.search.to).toBeUndefined();
  expect(router.state.location.search.name).toBe("model");
  expect(router.state.location.search.model).toBe("gpt-4o-mini");
  expect(screen.getByRole("combobox", { name: "Date range" }).textContent).toContain(
    "Last 24 hours",
  );
  await waitFor(() => {
    const filter = rpc.getObservationMetrics.mock.lastCall?.[0].filter;
    expect(Date.parse(filter.toTime) - Date.parse(filter.fromTime)).toBe(86400_000);
  });
});
it("cancels activity range selection with Escape and ignores old metrics after filters change", async () => {
  rpc.listObservations.mockResolvedValue(page());
  let resolveOld!: (value: unknown) => void;
  let oldSignal: AbortSignal | undefined;
  rpc.getObservationMetrics.mockImplementationOnce((_, options) => {
    oldSignal = options.signal;
    return new Promise((resolve) => {
      resolveOld = resolve;
    });
  });
  rpc.getObservationMetrics.mockResolvedValue({
    buckets: [
      create(ObservationMetricBucketSchema, {
        startTime: "2026-09-11T00:00:00Z",
        endTime: "2026-09-11T01:00:00Z",
        observationCount: 2n,
        errorCount: 2n,
        averageLatencySeconds: 1,
      }),
    ],
  });
  const router = mount("/tracing?range=custom&from=2026-09-11T00:00:00Z&to=2026-09-11T04:00:00Z");
  await screen.findByRole("row", { name: "Inspect Recipe model" });
  await waitFor(() => expect(resolveOld).toBeTypeOf("function"));
  fireEvent.click(screen.getByRole("tab", { name: "Errors" }));
  await waitFor(() =>
    expect(
      screen
        .getByRole("region", { name: "Trace activity" })
        .querySelectorAll('[data-errors="true"]').length,
    ).toBe(1),
  );
  expect(oldSignal?.aborted).toBe(true);
  await act(async () =>
    resolveOld({ buckets: [create(ObservationMetricBucketSchema, { observationCount: 999n })] }),
  );
  expect(
    screen
      .getByRole("region", { name: "Trace activity" })
      .querySelectorAll('[data-slot="activity-bar"]').length,
  ).toBe(1);
  const chart = within(screen.getByRole("region", { name: "Trace activity" })).getByRole("group", {
    name: "Observation count and average latency over time",
  });
  const before = router.state.location.search.from;
  fireEvent(chart, new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 300 }));
  fireEvent(chart, new MouseEvent("pointermove", { bubbles: true, clientX: 700 }));
  fireEvent.keyDown(chart, { key: "Escape" });
  fireEvent(chart, new MouseEvent("pointerup", { bubbles: true, clientX: 700 }));
  expect(router.state.location.search.from).toBe(before);
});

it("opens sessions as read-only chat with reasoning and tool results, and drills into individual traces", async () => {
  rpc.listObservations.mockResolvedValue(page());
  const captured = create(ObservationSchema, {
    ...observation,
    input: JSON.stringify([{ role: "user", content: "Look up the weather" }]),
    output: JSON.stringify({
      role: "assistant",
      content: [
        { type: "reasoning", text: "I will look up the current temperature." },
        {
          type: "tool-call",
          toolCallId: "weather-1",
          toolName: "weather",
          input: { city: "Berlin" },
        },
        {
          type: "tool-result",
          toolCallId: "weather-1",
          toolName: "weather",
          output: { temperature: 20 },
        },
        { type: "text", text: "The temperature is 20 degrees." },
      ],
    }),
  });
  rpc.getSession.mockResolvedValue(page([captured]));
  rpc.getTrace.mockResolvedValue(page([captured]));
  mount("/sessions?inspectSession=session-1");
  const conversation = await screen.findByRole("region", { name: "Session conversation" });
  await within(conversation).findByText("The temperature is 20 degrees.");
  expect(within(conversation).getByText("Look up the weather")).toBeTruthy();
  expect(within(conversation).getByRole("button", { name: /Thinking/ })).toBeTruthy();
  const tool = within(conversation).getByRole("article", { name: "Tool calls" });
  fireEvent.click(within(tool).getByRole("button", { name: "Weather" }));
  expect(within(tool).getByText(/"temperature": 20/)).toBeTruthy();
  expect(screen.queryByRole("toolbar", { name: "Waterfall view controls" })).toBeNull();
  expect(within(conversation).queryByRole("textbox")).toBeNull();
  fireEvent.click(within(conversation).getAllByRole("button", { name: "View trace" })[0]);
  await screen.findByRole("dialog", { name: "Trace details" });
  expect(rpc.getTrace).toHaveBeenCalledWith(
    expect.objectContaining({ traceId: "trace-1" }),
    expect.anything(),
  );
});
