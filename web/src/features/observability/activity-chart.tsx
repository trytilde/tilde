import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
export type ActivityBucket = {
  startTime: string;
  endTime: string;
  count: bigint;
  errorCount: bigint;
  averageLatencySeconds?: number;
};
import { date, duration, number } from "../tracing/format";

/** Shared histogram/brush UI; callers own the scoped metrics request. */
export function ActivityChart({
  fromTime,
  toTime,
  load,
  showLatency = false,
  kind,
  onRange,
  onClear,
  selectedRange,
}: {
  fromTime: string;
  toTime: string;
  load: (signal: AbortSignal, refresh: boolean) => Promise<ActivityBucket[]>;
  showLatency?: boolean;
  kind: "Trace" | "Log";
  onRange: (from: string, to: string) => void;
  onClear: () => void;
  selectedRange: { from: string; to: string } | null;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(1000);
  const [buckets, setBuckets] = useState<ActivityBucket[]>([]);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [hovered, setHovered] = useState<number | null>(null);
  const [draft, setSelection] = useState<{ start: number; end: number } | null>(null);
  const drag = useRef<{ start: number; end: number; edge: "start" | "end" | "new" } | null>(null);
  const selection =
    draft ??
    (selectedRange
      ? { start: Date.parse(selectedRange.from), end: Date.parse(selectedRange.to) }
      : null);
  useEffect(() => {
    const abort = new AbortController();
    setBusy(true);
    setError("");
    setBuckets([]);
    setSelection(null);
    drag.current = null;
    void load(abort.signal, retry > 0)
      .then((buckets) => {
        if (!abort.signal.aborted) setBuckets(buckets);
      })
      .catch((e: unknown) => {
        if (!abort.signal.aborted)
          setError(e instanceof Error ? e.message : "Activity unavailable");
      })
      .finally(() => {
        if (!abort.signal.aborted) setBusy(false);
      });
    return () => abort.abort();
  }, [load, retry]);
  useEffect(() => {
    if (!frame.current) return;
    const measure = () => setWidth(Math.max(240, frame.current?.clientWidth || 1000));
    const observer = new ResizeObserver(measure);
    observer.observe(frame.current);
    measure();
    return () => observer.disconnect();
  }, []);
  const from = Date.parse(fromTime),
    to = Date.parse(toTime);
  const left = 42,
    right = width - (showLatency ? 55 : 12),
    top = 8,
    bottom = 76;
  const countMax = Math.max(2, ...buckets.map((b) => Number(b.count)));
  const latencyMax = Math.max(0.001, ...buckets.map((b) => b.averageLatencySeconds ?? 0));
  const x = (time: number) => left + ((time - from) / (to - from)) * (right - left);
  const timeAt = (clientX: number, element: SVGSVGElement) => {
    const bounds = element.getBoundingClientRect();
    const px = ((clientX - bounds.left) / (bounds.width || width)) * width;
    return Math.round(from + Math.max(0, Math.min(1, (px - left) / (right - left))) * (to - from));
  };
  const indexAt = (time: number) =>
    buckets.findIndex((b) => time >= Date.parse(b.startTime) && time < Date.parse(b.endTime));
  const commit = (start: number, end: number) => {
    const a = Math.max(from, Math.min(start, end)),
      b = Math.min(to, Math.max(start, end));
    if (x(a) - left <= 2 && right - x(b) <= 2) onClear();
    else if (b > a) onRange(new Date(a).toISOString(), new Date(b).toISOString());
  };
  const applyBucket = (bucket: ActivityBucket) =>
    commit(Date.parse(bucket.startTime), Date.parse(bucket.endTime));
  const resize = (time: number) => {
    const active = drag.current!;
    if (active.edge === "start") return { start: Math.min(time, active.end - 1), end: active.end };
    if (active.edge === "end")
      return { start: active.start, end: Math.max(time, active.start + 1) };
    return { start: Math.min(active.start, time), end: Math.max(active.start, time) };
  };
  // Connect recorded averages; missing samples never become artificial zero latency.
  let path = "",
    connected = false;
  const points: { x: number; y: number; key: string }[] = [];
  for (const b of buckets) {
    if (b.averageLatencySeconds === undefined) continue;
    const px = x((Date.parse(b.startTime) + Date.parse(b.endTime)) / 2);
    const py = bottom - (b.averageLatencySeconds / latencyMax) * (bottom - top);
    path += `${connected ? "L" : "M"}${px},${py} `;
    connected = true;
    points.push({ x: px, y: py, key: b.startTime });
  }
  return (
    <div
      ref={frame}
      role="region"
      aria-label={`${kind} activity`}
      aria-busy={busy}
      className="trace-controls relative shrink-0 border-b bg-background"
    >
      <div className="flex h-6 items-center justify-end gap-3 overflow-hidden px-3 text-[10px] text-muted-foreground">
        <span className="mr-auto truncate">
          {selection
            ? `${date(new Date(selection.start).toISOString())} → ${date(new Date(selection.end).toISOString())}`
            : busy
              ? "Loading activity…"
              : ""}
        </span>
        <span className="shrink-0">
          <i className="mr-1 inline-block size-2 bg-primary/50" />
          Count
        </span>
        <span className="shrink-0">
          <i className="mr-1 inline-block size-2 bg-destructive" />
          Errors
        </span>
        {showLatency && (
          <span className="shrink-0 text-orange-600 dark:text-orange-400">— Avg latency (s)</span>
        )}
        <span className="shrink-0 border-l pl-3">Drag to filter time</span>
      </div>
      {error ? (
        <div
          role="alert"
          className="flex h-[102px] items-center justify-center gap-2 text-xs text-muted-foreground"
        >
          {error}
          <Button size="xs" variant="ghost" onClick={() => setRetry((value) => value + 1)}>
            Retry chart
          </Button>
        </div>
      ) : (
        <svg
          role="group"
          tabIndex={0}
          aria-label={
            showLatency ? "Observation count and average latency over time" : "Log count over time"
          }
          className="block w-full touch-none select-none"
          style={{ cursor: busy ? "default" : "crosshair" }}
          width={width}
          height={102}
          onPointerDown={(event) => {
            if (busy || event.button !== 0 || !buckets.length) return;
            const time = timeAt(event.clientX, event.currentTarget);
            const edge = (event.target as Element)
              .closest("[data-range-edge]")
              ?.getAttribute("data-range-edge");
            drag.current =
              selection && (edge === "start" || edge === "end")
                ? { ...selection, edge }
                : { start: time, end: time, edge: "new" };
            setSelection({ start: drag.current.start, end: drag.current.end });
            event.currentTarget.setPointerCapture?.(event.pointerId);
          }}
          onPointerMove={(event) => {
            const time = timeAt(event.clientX, event.currentTarget);
            setHovered(indexAt(time));
            if (drag.current) {
              setSelection(resize(time));
            }
          }}
          onPointerUp={(event) => {
            const range = drag.current;
            if (!range) return;
            const next = resize(timeAt(event.clientX, event.currentTarget));
            drag.current = null;
            setSelection(null);
            if (event.currentTarget.hasPointerCapture?.(event.pointerId))
              event.currentTarget.releasePointerCapture(event.pointerId);
            if (range.edge === "new" && x(next.end) - x(next.start) < 4) {
              const bucket = buckets[indexAt(next.start)];
              if (bucket) applyBucket(bucket);
            } else commit(next.start, next.end);
          }}
          onPointerCancel={() => {
            drag.current = null;
            setSelection(null);
          }}
          onPointerLeave={() => {
            if (!drag.current) setHovered(null);
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              drag.current = null;
              setSelection(null);
            }
          }}
        >
          {[0, 0.5, 1].map((ratio) => (
            <g key={ratio} aria-hidden="true">
              <line
                x1={left}
                x2={right}
                y1={bottom - ratio * (bottom - top)}
                y2={bottom - ratio * (bottom - top)}
                stroke="var(--border)"
                strokeDasharray="2 3"
              />
              <text
                x={left - 6}
                y={bottom - ratio * (bottom - top) + 3}
                textAnchor="end"
                fill="var(--muted-foreground)"
                fontSize={9}
              >
                {number(Math.ceil(countMax * ratio))}
              </text>
              {showLatency && (
                <text
                  x={right + 6}
                  y={bottom - ratio * (bottom - top) + 3}
                  fill="var(--muted-foreground)"
                  fontSize={9}
                >
                  {duration(latencyMax * ratio)}
                </text>
              )}
            </g>
          ))}
          {!busy &&
            buckets.map((bucket, index) => {
              const start = x(Date.parse(bucket.startTime)),
                end = x(Date.parse(bucket.endTime));
              const h = (Number(bucket.count) / countMax) * (bottom - top);
              return (
                <g key={bucket.startTime}>
                  <title>{`${date(bucket.startTime)} · ${number(Number(bucket.count))} ${kind === "Log" ? "logs" : "observations"} · ${number(Number(bucket.errorCount))} errors${showLatency ? ` · ${duration(bucket.averageLatencySeconds)} s` : ""}`}</title>
                  <rect
                    data-slot="activity-bar"
                    data-errors={bucket.errorCount > 0n}
                    x={start + 1}
                    y={bottom - h}
                    width={Math.max(1, end - start - 2)}
                    height={h}
                    fill={bucket.errorCount > 0n ? "var(--destructive)" : "var(--primary)"}
                    opacity={hovered === index ? 0.8 : 0.5}
                  />
                  <rect
                    role="button"
                    tabIndex={0}
                    aria-label={`${date(bucket.startTime)}: ${number(Number(bucket.count))} ${kind === "Log" ? "logs" : "observations"}, ${number(Number(bucket.errorCount))} errors${showLatency ? `, average latency ${duration(bucket.averageLatencySeconds)} seconds` : ""}. Filter to this interval.`}
                    x={start}
                    y={top}
                    width={Math.max(1, end - start)}
                    height={bottom - top}
                    fill="transparent"
                    onFocus={() => setHovered(index)}
                    onBlur={() => setHovered(null)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        applyBucket(bucket);
                      }
                    }}
                  />
                </g>
              );
            })}
          {showLatency && (
            <>
              <path
                data-slot="activity-latency"
                d={path}
                fill="none"
                stroke="var(--color-orange-500)"
                strokeWidth={1.5}
                className="pointer-events-none"
              />
              {points.map((p) => (
                <circle
                  key={p.key}
                  cx={p.x}
                  cy={p.y}
                  r={2}
                  fill="var(--color-orange-500)"
                  className="pointer-events-none"
                />
              ))}
            </>
          )}
          {[0, 0.25, 0.5, 0.75, 1].map((ratio) => (
            <text
              key={ratio}
              x={left + ratio * (right - left)}
              y={95}
              textAnchor={ratio === 0 ? "start" : ratio === 1 ? "end" : "middle"}
              fill="var(--muted-foreground)"
              fontSize={9}
            >
              {date(new Date(from + (to - from) * ratio).toISOString()).slice(0, 14)}
            </text>
          ))}
          {selection && (
            <g>
              <rect
                data-slot="activity-selection"
                x={x(selection.start)}
                y={top}
                width={Math.max(1, x(selection.end) - x(selection.start))}
                height={bottom - top}
                fill="var(--primary)"
                fillOpacity={0.12}
                stroke="var(--primary)"
                strokeWidth={1}
                className="pointer-events-none"
              />
              {(["start", "end"] as const).map((edge) => (
                <g
                  key={edge}
                  data-range-edge={edge}
                  role="slider"
                  tabIndex={0}
                  aria-label={edge === "start" ? "Selected range start" : "Selected range end"}
                  aria-valuemin={from}
                  aria-valuemax={to}
                  aria-valuenow={selection[edge]}
                  aria-valuetext={date(new Date(selection[edge]).toISOString())}
                  className="cursor-ew-resize outline-none focus-visible:stroke-ring"
                  onKeyDown={(event) => {
                    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
                    event.preventDefault();
                    event.stopPropagation();
                    const value =
                      event.key === "Home"
                        ? from
                        : event.key === "End"
                          ? to
                          : selection[edge] +
                            ((event.key === "ArrowLeft" ? -1 : 1) * (to - from)) / 100;
                    if (edge === "start")
                      commit(Math.max(from, Math.min(selection.end - 1, value)), selection.end);
                    else
                      commit(selection.start, Math.min(to, Math.max(selection.start + 1, value)));
                  }}
                >
                  <rect
                    x={x(selection[edge]) - 7}
                    y={top - 2}
                    width={14}
                    height={bottom - top + 4}
                    fill="transparent"
                  />
                  <rect
                    x={x(selection[edge]) - 2}
                    y={top}
                    width={4}
                    height={bottom - top}
                    rx={1}
                    fill="var(--primary)"
                  />
                </g>
              ))}
            </g>
          )}
        </svg>
      )}
    </div>
  );
}
