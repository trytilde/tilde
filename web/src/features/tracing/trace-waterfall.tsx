import { useEffect, useId, useMemo, useRef, useState } from "react";
import { TraceFold } from "@microcharts/react/trace-fold/interactive";
import type { Observation } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { Maximize, ZoomIn, ZoomOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useWaterfallNavigation } from "./use-waterfall-navigation";
import { observationKey } from "./records";
import { number } from "./format";

/** Parent-first ordering keeps concurrent siblings on separate visual lanes. */
function order(rows: Observation[]) {
  const known = new Set(rows.map(observationKey));
  const children = new Map<string, Observation[]>();
  for (const row of rows) {
    const parent = `${row.traceId}:${row.parentId}`;
    const key = row.parentId && known.has(parent) ? parent : "";
    children.set(key, [...(children.get(key) ?? []), row]);
  }
  for (const items of children.values())
    items.sort((a, b) => a.startTime.localeCompare(b.startTime) || a.id.localeCompare(b.id));
  const seen = new Set<string>(),
    result: Observation[] = [];
  function visit(row: Observation) {
    const key = observationKey(row);
    if (seen.has(key)) return;
    seen.add(key);
    result.push(row);
    for (const child of children.get(key) ?? []) visit(child);
  }
  for (const row of children.get("") ?? []) visit(row);
  for (const row of rows) visit(row); // Orphans/cycles remain inspectable.
  return result;
}

/** TraceFold has a 40-span cap. Render aligned real-span batches on one shared
 * millisecond scale, so longer traces are neither truncated nor independently stretched.
 */
export function TraceWaterfall({
  rows,
  selected,
  onSelect,
  onHover,
  onLoadMore,
  hasMore,
  busy,
  fullHeight = false,
}: {
  rows: Observation[];
  selected: string;
  onSelect: (key: string) => void;
  onHover: (key: string | null) => void;
  onLoadMore: () => void;
  hasMore: boolean;
  busy: boolean;
  fullHeight?: boolean;
}) {
  const pointerTime = useRef<number | null>(null);
  const [viewport, setViewport] = useState({ width: 800, height: 360 });
  const [hovered, setHovered] = useState<string | null>(null);
  const [bucket, setBucket] = useState<number | null>(null);
  const spans = useMemo(() => {
    const timed = order(rows).filter((row) => Number.isFinite(Date.parse(row.startTime)));
    const origin = Math.min(...timed.map((row) => Date.parse(row.startTime)));
    return timed.map((row) => {
      const start = Date.parse(row.startTime);
      const end = Date.parse(row.endTime);
      return {
        row,
        start: Math.max(0, start - origin),
        duration: Number.isFinite(end)
          ? Math.max(0, end - start)
          : Math.max(0, (row.latencySeconds ?? 0) * 1000),
      };
    });
  }, [rows]);
  const elapsed = Math.max(0, ...spans.map((span) => span.start + span.duration));
  const total = Math.ceil((elapsed + Math.max(500, elapsed * 0.15)) / 500) * 500;
  const navigation = useWaterfallNavigation(
    spans.length > 0,
    Math.min(Math.max(64, total / 500), 4_000_000 / viewport.width),
    () => {
      setHovered(null);
      onHover(null);
      setBucket(null);
    },
  );
  const { frame, scroll, camera, dragged, pointerStart } = navigation;
  const scope = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  useEffect(() => {
    if (!frame.current) return;
    const element = frame.current;
    const measure = () =>
      setViewport({
        width: scroll.current?.clientWidth || element.clientWidth || 800,
        height: element.clientHeight || 360,
      });
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    if (scroll.current) observer.observe(scroll.current);
    measure();
    return () => observer.disconnect();
  }, [spans.length]);
  const width = viewport.width;
  const timelineWidth = width * camera.scale;
  const scale = timelineWidth / total;
  // Include label overhang and panning room beyond the final span. This space also
  // belongs to the time axis, so guides remain truthful all the way to the edge.
  const labelEnd = Math.max(
    0,
    ...spans.map(({ row, start }) => start * scale + (row.name || row.type).length * 7 + 16),
  );
  const plotWidth = Math.ceil(Math.max(timelineWidth, labelEnd) + width / 4);
  const endTime = plotWidth / scale;
  // Zoom only the time axis. Rows and labels keep their dense, readable size.
  const rowHeight = 26;
  const height = spans.length * rowHeight;
  const chunks = useMemo(() => {
    const chunks = [];
    for (let index = 0; index < spans.length; index += 40) {
      const items = spans.slice(index, index + 40);
      const start = Math.min(...items.map((span) => span.start));
      const end = Math.max(...items.map((span) => span.start + span.duration));
      chunks.push({
        index,
        items,
        left: start * scale - 1,
        width: Math.max(3, (end - start) * scale + 2),
        height: items.length * rowHeight + 2,
        data: items.map((span, lane) => ({
          label: span.row.name || span.row.type,
          start: span.start,
          duration: span.duration,
          depth: lane,
        })),
      });
    }
    return chunks;
  }, [spans, scale, rowHeight]);
  // Keep 500 ms guides, spacing labels further apart when fitting long traces.
  const screenScale = scale;
  const tickStep = Math.max(1, Math.ceil(64 / (500 * screenScale))) * 500;
  const firstTick = Math.max(0, Math.floor(-camera.positionX / screenScale / tickStep));
  const lastTick = Math.min(
    Math.floor(endTime / tickStep),
    Math.ceil((width - camera.positionX) / screenScale / tickStep),
  );
  const ticks = Array.from(
    { length: Math.max(0, lastTick - firstTick + 1) },
    (_, i) => (firstTick + i) * tickStep,
  );
  const measuredKey = hovered ?? selected;
  const measuredIndex = spans.findIndex((span) => observationKey(span.row) === measuredKey);
  const measuredSpan = spans[measuredIndex];
  // Float the ruler below the bar without changing any lane or bar geometry.
  const rulerHeight = 14;
  const measuredLabel = measuredSpan ? `${number(measuredSpan.duration)} ms` : "";
  const rulerFont = Math.min(10, rulerHeight * 0.85);
  const labelWidth = measuredLabel.length * rulerFont * 0.61 + 8;
  const rulerWidth = measuredSpan
    ? Math.min(width - 4, Math.max(measuredSpan.duration * screenScale, labelWidth + 16))
    : 0;
  const rulerLeft = measuredSpan
    ? Math.max(
        2,
        Math.min(
          width - rulerWidth - 2,
          camera.positionX +
            (measuredSpan.start + measuredSpan.duration / 2) * screenScale -
            rulerWidth / 2,
        ),
      )
    : 0;
  const rulerTop = camera.positionY + (measuredIndex + 1) * rowHeight;
  const rulerVisible =
    measuredSpan &&
    rulerTop >= 0 &&
    rulerTop <= viewport.height - 24 &&
    camera.positionX + (measuredSpan.start + measuredSpan.duration) * screenScale >= 0 &&
    camera.positionX + measuredSpan.start * screenScale <= width;
  function loadAtEnd() {
    const element = scroll.current;
    if (
      element &&
      hasMore &&
      !busy &&
      element.scrollHeight - element.scrollTop - element.clientHeight < 104
    )
      onLoadMore();
  }
  if (!spans.length)
    return (
      <div className="border-b p-4 text-xs text-muted-foreground">
        {busy ? "Loading trace…" : "No timed observations recorded."}
      </div>
    );
  return (
    <section
      aria-label="Trace waterfall"
      className={
        fullHeight ? "relative flex min-h-0 flex-1 flex-col" : "relative shrink-0 border-b"
      }
    >
      <div className="flex h-7 shrink-0 items-center justify-between gap-2 overflow-hidden border-b bg-muted/20 px-3 text-[10px] text-muted-foreground">
        <span data-slot="waterfall-summary" className="truncate">
          {`${spans.length} spans · ${number(elapsed)} ms`}
        </span>
        <span className="flex shrink-0 items-center gap-3">
          <span className="flex items-center gap-1">
            <i className="size-2 rounded-sm bg-amber-500" />≥ 500 ms
          </span>
          <span className="flex items-center gap-1">
            <i className="size-2 rounded-sm bg-destructive" />
            &gt; 1,000 ms
          </span>
        </span>
      </div>
      <div
        ref={frame}
        className={`trace-waterfall relative flex w-full flex-col overflow-hidden ${fullHeight ? "min-h-0 flex-1" : "max-h-[42svh]"}`}
        style={fullHeight ? undefined : { height: Math.min(360, spans.length * 26 + 28) }}
      >
        <div
          className="relative z-20 h-6 shrink-0 overflow-hidden border-b bg-popover"
          aria-label="Time in milliseconds"
        >
          {ticks.map((tick) => (
            <span
              key={tick}
              className="absolute top-1 whitespace-nowrap text-[10px] text-muted-foreground"
              style={{
                left: camera.positionX + tick * screenScale,
                transform: tick * scale >= plotWidth - 1 ? "translateX(-100%)" : "translateX(3px)",
              }}
            >
              {number(tick)} ms
            </span>
          ))}
        </div>
        <div
          ref={scroll}
          className="waterfall-scroll min-h-0 flex-1 overflow-auto overscroll-contain cursor-grab active:cursor-grabbing"
          aria-label="Pan and zoom waterfall"
          tabIndex={0}
          onScroll={() => {
            const movedDown = (scroll.current?.scrollTop ?? 0) > -camera.positionY;
            navigation.onScroll();
            if (movedDown) loadAtEnd();
          }}
          onWheelCapture={(event) => {
            if (!event.ctrlKey && event.deltaY > 0) loadAtEnd();
          }}
          onPointerDownCapture={(event) => {
            dragged.current = false;
            pointerStart.current = {
              x: event.clientX,
              y: event.clientY,
              left: event.currentTarget.scrollLeft,
              top: event.currentTarget.scrollTop,
            };
          }}
          onPointerMoveCapture={(event) => {
            if (event.pointerType === "touch" || !(event.buttons & 5)) return;
            const dx = event.clientX - pointerStart.current.x,
              dy = event.clientY - pointerStart.current.y;
            if (Math.hypot(dx, dy) <= 4 && !dragged.current) return;
            dragged.current = true;
            event.preventDefault();
            event.currentTarget.setPointerCapture?.(event.pointerId);
            event.currentTarget.style.userSelect = "none";
            event.currentTarget.scrollLeft = pointerStart.current.left - dx;
            event.currentTarget.scrollTop = pointerStart.current.top - dy;
            navigation.onScroll();
          }}
          onPointerUp={(event) => {
            event.currentTarget.style.userSelect = "";
            if (event.currentTarget.hasPointerCapture?.(event.pointerId))
              event.currentTarget.releasePointerCapture(event.pointerId);
          }}
          onPointerCancel={(event) => {
            event.currentTarget.style.userSelect = "";
          }}
          onClickCapture={(event) => {
            if (dragged.current && event.detail > 0) {
              event.preventDefault();
              event.stopPropagation();
            }
          }}
        >
          <div
            data-slot="waterfall-plot"
            className="relative"
            style={{ height, width: plotWidth, overflow: "clip" }}
            onPointerMoveCapture={(event) => {
              if (event.buttons || (event.pointerType === "touch" && dragged.current)) return;
              const bounds = event.currentTarget.getBoundingClientRect();
              const x = event.clientX - bounds.left;
              const span = spans[Math.floor((event.clientY - bounds.top) / rowHeight)];
              const key = span ? observationKey(span.row) : null;
              setHovered(key);
              onHover(key);
              pointerTime.current = Math.min(endTime - 1, Math.max(0, x / scale));
              setBucket(Math.floor(pointerTime.current / 500) * 500);
            }}
            onClick={(event) => {
              if (event.detail === 0) return; // Keyboard selection is handled by TraceFold.
              const y = event.clientY - event.currentTarget.getBoundingClientRect().top;
              const span = spans[Math.floor(y / rowHeight)];
              if (span) onSelect(observationKey(span.row));
            }}
            onPointerLeave={() => {
              pointerTime.current = null;
              setHovered(null);
              onHover(null);
              setBucket(null);
            }}
            onKeyDownCapture={() => {
              dragged.current = false;
              pointerTime.current = null;
            }}
          >
            {hovered && bucket !== null && (
              <div
                data-slot="time-highlight"
                className="pointer-events-none absolute inset-y-0 bg-primary/5"
                style={{ left: bucket * scale, width: 500 * scale }}
              />
            )}
            <svg
              className="pointer-events-none absolute inset-0"
              width={plotWidth}
              height={height}
              aria-hidden="true"
            >
              <defs>
                <pattern
                  id={`trace-grid-${scope}`}
                  width={500 * scale}
                  height={6}
                  patternUnits="userSpaceOnUse"
                >
                  <path d="M 0 0 V 3" stroke="var(--border)" />
                </pattern>
              </defs>
              <rect width={plotWidth} height={height} fill={`url(#trace-grid-${scope})`} />
            </svg>
            {chunks.map((chunk) => {
              const chartClass = `trace-fold-${scope}-${chunk.index}`;
              const selectedIndex = chunk.items.findIndex(
                (span) => observationKey(span.row) === selected,
              );
              return (
                <div
                  key={chunk.index}
                  // SVG aspect-ratio fitting must never scale the vertical lanes.
                  ref={(node) =>
                    node?.querySelector("svg.mc-root")?.setAttribute("preserveAspectRatio", "none")
                  }
                  className={`trace-fold-chunk absolute ${chartClass}`}
                  style={{
                    top: chunk.index * rowHeight,
                    left: chunk.left,
                    width: chunk.width,
                    height: chunk.height,
                  }}
                >
                  <style>
                    {chunk.items
                      .map((span, index) => {
                        const key = observationKey(span.row);
                        const color =
                          (span.row.latencySeconds ?? span.duration / 1000) > 1
                            ? "var(--destructive)"
                            : (span.row.latencySeconds ?? span.duration / 1000) >= 0.5
                              ? "#f59e0b"
                              : "var(--primary)";
                        return `.${chartClass} .mc-trace > rect[data-mc-ink]:nth-of-type(${index + 1}) { fill: color-mix(in oklab, ${color} ${key === hovered ? 20 : key === selected ? 50 : 35}%, var(--background)); fill-opacity: 1; }`;
                      })
                      .join("\n")}
                  </style>
                  <TraceFold
                    data={chunk.data}
                    width={chunk.width}
                    height={chunk.height}
                    style={{ width: chunk.width, height: chunk.height }}
                    labels={false}
                    emphasis="none"
                    readout={false}
                    title="Trace spans"
                    selectedIndex={selectedIndex < 0 ? null : selectedIndex}
                    format={(value) => `${number(value)} ms`}
                    onActive={(value) => {
                      if (pointerTime.current !== null) return;
                      const span = value ? chunk.items[value.index] : undefined;
                      const key = span ? observationKey(span.row) : null;
                      setHovered(key);
                      onHover(key);
                      if (span)
                        setBucket(Math.floor((pointerTime.current ?? span.start) / 500) * 500);
                    }}
                    onSelect={(value) => {
                      if (value && !dragged.current)
                        onSelect(observationKey(chunk.items[value.index].row));
                    }}
                  >
                    {chunk.items.map((span, index) => {
                      const x = span.start * scale + 4 - chunk.left;
                      const clip = `trace-label-${scope}-${chunk.index}-${index}`;
                      return (
                        <g key={observationKey(span.row)} className="pointer-events-none">
                          <defs>
                            <clipPath id={clip}>
                              <rect
                                x={x}
                                y={index * rowHeight}
                                width={Math.max(0, plotWidth - chunk.left - x)}
                                height={rowHeight}
                              />
                            </clipPath>
                          </defs>
                          <text
                            x={x}
                            y={index * rowHeight + rowHeight / 2 + 1}
                            fontSize={11}
                            dominantBaseline="middle"
                            clipPath={`url(#${clip})`}
                            className="fill-foreground"
                            style={{
                              fontFamily: "IBM Plex Mono, monospace",
                              fontWeight: observationKey(span.row) === hovered ? 700 : 400,
                            }}
                          >
                            {span.row.name || span.row.type}
                          </text>
                        </g>
                      );
                    })}
                  </TraceFold>
                </div>
              );
            })}
          </div>
        </div>
      </div>
      <div
        role="toolbar"
        aria-label="Waterfall view controls"
        className="trace-controls absolute right-2 top-[56px] z-40 flex items-center gap-0.5 rounded-md border bg-popover/95 p-0.5 shadow-sm"
      >
        {[
          {
            label: "Zoom out",
            Icon: ZoomOut,
            action: () => navigation.zoomOut(),
            disabled: camera.scale <= 1,
          },
          {
            label: "Zoom in",
            Icon: ZoomIn,
            action: () => navigation.zoomIn(),
            disabled: false,
          },
          {
            label: "Fit timeline",
            Icon: Maximize,
            action: () => navigation.fit(),
            disabled: false,
          },
        ].map(({ label, Icon, action, disabled }) => (
          <Tooltip key={label}>
            <TooltipTrigger
              render={
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  disabled={disabled}
                  aria-label={label}
                  onClick={action}
                >
                  <Icon />
                </Button>
              }
            />
            <TooltipContent className="trace-controls">{label}</TooltipContent>
          </Tooltip>
        ))}
      </div>
      {(hasMore || busy) && (
        <div className="absolute bottom-2 left-2 z-30 bg-popover/95 text-[10px]">
          {busy ? (
            <span role="status">Loading more spans…</span>
          ) : (
            <Button size="xs" variant="ghost" onClick={onLoadMore}>
              Load more spans
            </Button>
          )}
        </div>
      )}
      {rulerVisible && (
        <svg
          data-slot="span-duration"
          aria-label={`${measuredSpan.row.name}: ${measuredLabel}`}
          className="pointer-events-none absolute top-[52px] left-0 z-30 overflow-visible"
          width={width}
          height={1}
        >
          <g transform={`translate(${rulerLeft}, ${rulerTop + rulerHeight / 2 + 2})`}>
            <path
              d={`M 0 0 H ${rulerWidth} M 3 -3 L 0 0 L 3 3 M ${rulerWidth - 3} -3 L ${rulerWidth} 0 L ${rulerWidth - 3} 3`}
              fill="none"
              stroke="var(--foreground)"
              strokeWidth={0.75}
            />
            <rect
              x={(rulerWidth - labelWidth) / 2}
              y={-rulerHeight / 2}
              width={labelWidth}
              height={rulerHeight}
              fill="var(--background)"
            />
            <text
              x={rulerWidth / 2}
              y={0}
              dominantBaseline="middle"
              textAnchor="middle"
              fontSize={rulerFont}
              fill="var(--foreground)"
              style={{ fontFamily: "IBM Plex Mono, monospace", fontWeight: 400 }}
            >
              {measuredLabel}
            </text>
          </g>
        </svg>
      )}
    </section>
  );
}
