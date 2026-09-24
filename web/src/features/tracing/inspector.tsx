import { mergeObservations, observationKey } from "./records";
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { traces } from "@/client";
import type {
  Observation,
  ObservationFilter,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { Sheet, SheetContent, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Maximize2, Minimize2 } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import { TraceWaterfall } from "./trace-waterfall";
const SessionConversation = lazy(() =>
  import("./session-conversation").then((module) => ({ default: module.SessionConversation })),
);
import { TracePayload } from "./trace-payload";
import { cost, date, duration, number } from "./format";
import { ExternalLink } from "./links";
export function Inspector({
  agentId,
  traceId,
  sessionId,
  selectedId,
  filter,
  initialObservation,
  onClose,
}: {
  agentId: string;
  traceId?: string;
  sessionId?: string;
  selectedId?: string;
  filter: Partial<Omit<ObservationFilter, "$typeName">>;
  initialObservation?: Observation;
  onClose: () => void;
}) {
  const [rows, setRows] = useState<Observation[]>(initialObservation ? [initialObservation] : []);
  const [cursor, setCursor] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState(traceId && selectedId ? `${traceId}:${selectedId}` : "");
  const [fullScreen, setFullScreen] = useState(false);
  const [inspected, setInspected] = useState<Observation>();
  const [hovered, setHovered] = useState<string | null>(null);
  const [detailTab, setDetailTab] = useState("input");
  const requestController = useRef<AbortController | null>(null);
  const loading = useRef(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    requestController.current = abort;
    loading.current = true;
    setBusy(true);
    setError("");
    setRows(initialObservation ? [initialObservation] : []);
    setCursor("");
    const request = sessionId
      ? traces.getSession(
          { agentId, sessionId, filter, refresh: attempt > 0 },
          { signal: abort.signal },
        )
      : traces.getTrace({ agentId, traceId, refresh: attempt > 0 }, { signal: abort.signal });
    void request
      .then((page) => {
        if (abort.signal.aborted) return;
        setRows(
          mergeObservations(page.observations, initialObservation ? [initialObservation] : []),
        );
        setCursor(page.nextCursor);
        setSelected(
          (id) => id || (page.observations[0] ? observationKey(page.observations[0]) : ""),
        );
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(e.message);
      })
      .finally(() => {
        if (!abort.signal.aborted) {
          loading.current = false;
          setBusy(false);
        }
      });
    return () => {
      abort.abort();
      requestController.current?.abort();
    };
  }, [agentId, traceId, sessionId, attempt]);
  async function more() {
    if (loading.current || !cursor) return;
    loading.current = true;
    const abort = new AbortController();
    requestController.current = abort;
    setBusy(true);
    setError("");
    try {
      const page = sessionId
        ? await traces.getSession({ agentId, sessionId, filter, cursor }, { signal: abort.signal })
        : await traces.getTrace({ agentId, traceId, cursor }, { signal: abort.signal });
      if (abort.signal.aborted) return;
      setRows((old) => mergeObservations(old, page.observations));
      setCursor(page.nextCursor === cursor ? "" : page.nextCursor);
    } catch (error) {
      if (!abort.signal.aborted)
        setError(error instanceof Error ? error.message : "Unable to load observations");
    } finally {
      if (!abort.signal.aborted) {
        loading.current = false;
        setBusy(false);
      }
    }
  }
  const activeKey = hovered ?? selected;
  const active = rows.find((r) => observationKey(r) === activeKey);
  const metadata = active
    ? JSON.stringify({
        traceId: active.traceId,
        sessionId: active.sessionId,
        invocationId: active.invocationId,
        type: active.type,
        level: active.level,
        attributes: Object.fromEntries(
          active.attributes.map((attribute) => {
            let value: unknown = attribute.value;
            try {
              value = JSON.parse(attribute.value);
            } catch {
              /* Preserve plain attribute values. */
            }
            return [attribute.key, value];
          }),
        ),
      })
    : "";
  return (
    <>
      <Sheet
        open
        onOpenChange={(open) => {
          if (!open) onClose();
        }}
      >
        <SheetContent
          side="right"
          className={`w-full! gap-0 overflow-hidden p-0 sm:max-w-none! ${fullScreen ? "" : "md:w-[min(960px,85vw)]!"}`}
        >
          <header className="flex h-12 shrink-0 items-center justify-between gap-3 border-b px-3 pr-12">
            <div className="min-w-0">
              <SheetTitle className="text-sm">{sessionId ? "Session" : "Trace"} details</SheetTitle>
              <SheetDescription className="sr-only">{sessionId || traceId}</SheetDescription>
            </div>
            <div className="flex items-center gap-1">
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={fullScreen ? "Exit full screen" : "Full screen"}
                      aria-pressed={fullScreen}
                      onClick={() => setFullScreen((value) => !value)}
                    >
                      {fullScreen ? <Minimize2 /> : <Maximize2 />}
                    </Button>
                  }
                />
                <TooltipContent>{fullScreen ? "Exit full screen" : "Full screen"}</TooltipContent>
              </Tooltip>
              <ExternalLink
                href={sessionId ? rows[0]?.sessionUrl : rows[0]?.traceUrl}
                label={sessionId ? "Open session in Langfuse" : "Open trace in Langfuse"}
                compact
              />
            </div>
          </header>
          {error && (
            <div
              role="alert"
              className="flex shrink-0 items-center gap-2 border-b px-3 py-1.5 text-xs text-destructive"
            >
              {error}
              <Button
                type="button"
                size="xs"
                variant="outline"
                onClick={() => setAttempt((n) => n + 1)}
              >
                Retry
              </Button>
            </div>
          )}
          {sessionId ? (
            <Suspense
              fallback={
                <p role="status" className="p-4 text-xs">
                  Loading session…
                </p>
              }
            >
              <SessionConversation
                rows={rows}
                busy={busy}
                hasMore={!!cursor}
                onLoadMore={() => void more()}
                onInspect={(id) => setInspected(rows.find((row) => observationKey(row) === id))}
              />
            </Suspense>
          ) : (
            <div
              data-slot="trace-inspector-layout"
              className={
                fullScreen ? "grid min-h-0 flex-1 grid-cols-2" : "flex min-h-0 flex-1 flex-col"
              }
            >
              <div className={fullScreen ? "flex min-h-0 min-w-0 flex-col border-r" : "contents"}>
                <TraceWaterfall
                  fullHeight={fullScreen}
                  rows={rows}
                  selected={selected}
                  onSelect={setSelected}
                  onHover={setHovered}
                  onLoadMore={() => void more()}
                  hasMore={!!cursor && !error}
                  busy={busy}
                />
              </div>
              <div className="flex min-h-0 min-w-0 flex-1 flex-col">
                {active ? (
                  <>
                    <section
                      aria-label="Selected observation"
                      className="shrink-0 space-y-2 px-3 py-2"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <h3 className="min-w-0 truncate text-sm font-semibold" title={active.name}>
                          {active.name}
                        </h3>
                        <ExternalLink
                          href={active.observationUrl}
                          label="Open observation in Langfuse"
                          compact
                        />
                      </div>
                      <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[11px] sm:grid-cols-4">
                        {[
                          ["Model", active.model || "—"],
                          ["Latency (s)", duration(active.latencySeconds)],
                          ["Tokens", number(active.totalTokens)],
                          ["Cost (USD)", cost(active.costUsd)],
                          ["Started", date(active.startTime)],
                          ...(active.timeToFirstTokenSeconds !== undefined
                            ? [["First token (s)", duration(active.timeToFirstTokenSeconds)]]
                            : []),
                          ["Input tokens", number(active.inputTokens)],
                          ["Output tokens", number(active.outputTokens)],
                        ].map(([label, value]) => (
                          <div key={label} className="min-w-0">
                            <dt className="text-muted-foreground">{label}</dt>
                            <dd
                              className={`truncate font-mono ${label === "Latency (s)" && (active.latencySeconds ?? 0) > 1 ? "text-destructive" : ""}`}
                              title={value}
                            >
                              {value}
                            </dd>
                          </div>
                        ))}
                      </dl>
                      {active.statusMessage && (
                        <p className="rounded bg-destructive/5 px-2 py-1 text-[11px] text-destructive">
                          {active.statusMessage}
                        </p>
                      )}
                    </section>
                    <Tabs
                      value={detailTab}
                      onValueChange={(tab) => setDetailTab(String(tab))}
                      className="min-h-0 flex-1 gap-0"
                    >
                      <TabsList
                        aria-label="Observation details"
                        className="h-8 w-full shrink-0 justify-start gap-0 rounded-none border-y bg-muted/20 p-0"
                      >
                        {[
                          ["input", "Input"],
                          ["output", "Output"],
                          ["metadata", "Metadata"],
                        ].map(([value, label]) => (
                          <TabsTrigger
                            key={value}
                            value={value}
                            className="h-8 rounded-none border-b-2 border-transparent px-3 text-xs data-active:border-primary data-active:bg-transparent data-active:shadow-none"
                          >
                            {label}
                          </TabsTrigger>
                        ))}
                      </TabsList>
                      <TabsContent value="input" className="min-h-0 flex-1 overflow-auto p-3">
                        <TracePayload key={`${activeKey}:input`} text={active.input} />
                      </TabsContent>
                      <TabsContent value="output" className="min-h-0 flex-1 overflow-auto p-3">
                        <TracePayload key={`${activeKey}:output`} text={active.output} />
                      </TabsContent>
                      <TabsContent value="metadata" className="min-h-0 flex-1 overflow-auto p-3">
                        <TracePayload key={`${activeKey}:metadata`} text={metadata} />
                      </TabsContent>
                    </Tabs>
                  </>
                ) : (
                  <p className="p-3 text-xs text-muted-foreground">
                    {busy
                      ? "Loading trace…"
                      : "Select a span in the chart. Scroll the chart to load more spans."}
                  </p>
                )}
              </div>
            </div>
          )}
        </SheetContent>
      </Sheet>
      {sessionId && inspected && (
        <Inspector
          agentId={agentId}
          traceId={inspected.traceId}
          selectedId={inspected.id}
          initialObservation={inspected}
          filter={filter}
          onClose={() => setInspected(undefined)}
        />
      )}
    </>
  );
}
