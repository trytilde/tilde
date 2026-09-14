import { mergeObservations, observationKey } from "./records";
import { useEffect, useMemo, useState } from "react";
import { traces } from "@/client";
import type {
  Observation,
  ObservationFilter,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { Dialog, DialogContent, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { IOPreview, ViewModeToggle, type ViewMode } from "./io-preview";
import { cost, date, duration, number } from "./format";
import { ExternalLink } from "./links";
export function Inspector({
  agentId,
  traceId,
  sessionId,
  selectedId,
  filter,
  onClose,
}: {
  agentId: string;
  traceId?: string;
  sessionId?: string;
  selectedId?: string;
  filter: Partial<Omit<ObservationFilter, "$typeName">>;
  onClose: () => void;
}) {
  const [rows, setRows] = useState<Observation[]>([]);
  const [cursor, setCursor] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState(traceId && selectedId ? `${traceId}:${selectedId}` : "");
  const [mode, setMode] = useState<ViewMode>("pretty");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    setBusy(true);
    setError("");
    setRows([]);
    setCursor("");
    const request = sessionId
      ? traces.getSession(
          { agentId, sessionId, filter, refresh: attempt > 0 },
          { signal: abort.signal },
        )
      : traces.getTrace({ agentId, traceId, refresh: attempt > 0 }, { signal: abort.signal });
    void request
      .then((page) => {
        setRows(mergeObservations([], page.observations));
        setCursor(page.nextCursor);
        setSelected(
          (id) => id || (page.observations[0] ? observationKey(page.observations[0]) : ""),
        );
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(e.message);
      })
      .finally(() => {
        if (!abort.signal.aborted) setBusy(false);
      });
    return () => abort.abort();
  }, [agentId, traceId, sessionId, attempt]);
  async function more() {
    setBusy(true);
    setError("");
    try {
      const page = sessionId
        ? await traces.getSession({ agentId, sessionId, filter, cursor })
        : await traces.getTrace({ agentId, traceId, cursor });
      setRows((old) => mergeObservations(old, page.observations));
      setCursor(page.nextCursor);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Unable to load observations");
    } finally {
      setBusy(false);
    }
  }
  const ordered = useMemo(() => {
    const byId = new Map(rows.map((r) => [observationKey(r), r]));
    const depth = (row: Observation) => {
      let n = 0,
        parent = row.parentId;
      const seen = new Set([row.id]);
      while (parent && byId.has(`${row.traceId}:${parent}`) && !seen.has(parent) && n < 32) {
        seen.add(parent);
        n++;
        parent = byId.get(`${row.traceId}:${parent}`)!.parentId;
      }
      return n;
    };
    return rows
      .map((row) => ({
        row,
        depth: depth(row),
        missing: !!row.parentId && !byId.has(`${row.traceId}:${row.parentId}`),
      }))
      .sort((a, b) => a.row.startTime.localeCompare(b.row.startTime));
  }, [rows]);
  const active = rows.find((r) => observationKey(r) === selected);
  const start = Math.min(...rows.map((r) => Date.parse(r.startTime)).filter(Number.isFinite));
  const end = Math.max(
    ...rows.map((r) => Date.parse(r.endTime || r.startTime)).filter(Number.isFinite),
  );
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="flex h-[88vh] max-w-[96vw] flex-col gap-3 sm:max-w-6xl">
        <div className="flex items-start justify-between gap-3 pr-8">
          <div>
            <DialogTitle>{sessionId ? "Session" : "Trace"} details</DialogTitle>
            <DialogDescription className="break-all font-mono text-xs">
              {sessionId || traceId}
            </DialogDescription>
          </div>
          <ExternalLink
            href={sessionId ? rows[0]?.sessionUrl : rows[0]?.traceUrl}
            label={sessionId ? "Open session in Langfuse" : "Open trace in Langfuse"}
          />
        </div>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}{" "}
            <Button
              type="button"
              size="xs"
              variant="outline"
              onClick={() => setAttempt((n) => n + 1)}
            >
              Retry
            </Button>
          </p>
        )}
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span>
            {rows.length} observations loaded{cursor ? " · Partial results" : ""}
          </span>
          {cursor && (
            <Button
              type="button"
              size="xs"
              variant="outline"
              disabled={busy}
              onClick={() => void more()}
            >
              Load more
            </Button>
          )}
          {busy && <span role="status">Loading…</span>}
        </div>
        <div className="grid min-h-0 flex-1 overflow-hidden rounded-lg border md:grid-cols-[minmax(220px,35%)_1fr]">
          <aside
            className="overflow-auto border-r bg-muted/20 p-2"
            aria-label="Span hierarchy and timeline"
          >
            {ordered.map(({ row, depth, missing }) => (
              <button
                type="button"
                key={`${row.traceId}:${row.id}`}
                onClick={() => setSelected(observationKey(row))}
                className={`mb-1 block w-full rounded-md p-2 text-left text-xs hover:bg-muted ${active && observationKey(active) === observationKey(row) ? "bg-muted ring-1 ring-border" : ""}`}
                style={{ paddingLeft: 8 + depth * 12 }}
              >
                <span className="flex justify-between gap-2">
                  <span
                    className={`truncate font-medium ${row.level === "ERROR" ? "text-destructive" : ""}`}
                  >
                    {row.name || row.type}
                  </span>
                  <span>{duration(row.latencySeconds)}</span>
                </span>
                <span className="text-[10px] text-muted-foreground">
                  {row.type}
                  {missing ? " · Parent outside loaded results" : ""}
                </span>
                <div className="mt-1 h-1 rounded bg-muted">
                  <div
                    className="h-1 rounded bg-primary/60"
                    style={{
                      marginLeft: `${Math.max(0, ((Date.parse(row.startTime) - start) / Math.max(1, end - start)) * 100)}%`,
                      width: `${Math.max(1, ((Date.parse(row.endTime || row.startTime) - Date.parse(row.startTime)) / Math.max(1, end - start)) * 100)}%`,
                    }}
                  />
                </div>
              </button>
            ))}
          </aside>
          <main className="min-h-0 space-y-4 overflow-auto p-4">
            {active ? (
              <>
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <h3 className="font-semibold">{active.name}</h3>
                  <ExternalLink href={active.observationUrl} label="Open observation in Langfuse" />
                </div>
                <div className="grid grid-cols-2 gap-3 text-xs md:grid-cols-4">
                  {[
                    ["Model", active.model || "—"],
                    ["Latency", duration(active.latencySeconds)],
                    ["Tokens", number(active.totalTokens)],
                    ["Cost", cost(active.costUsd)],
                    ["Started", date(active.startTime)],
                    ["TTFT", duration(active.timeToFirstTokenSeconds)],
                    ["Input tokens", number(active.inputTokens)],
                    ["Output tokens", number(active.outputTokens)],
                  ].map(([name, value]) => (
                    <div key={name}>
                      <p className="text-muted-foreground">{name}</p>
                      <p className="mt-1 break-words font-medium">{value}</p>
                    </div>
                  ))}
                </div>
                {active.statusMessage && (
                  <p className="rounded border border-destructive/20 bg-destructive/5 p-3 text-sm text-destructive">
                    {active.statusMessage}
                  </p>
                )}
                <ViewModeToggle selectedView={mode} onViewChange={setMode} />
                <IOPreview title="Input" text={active.input} mode={mode} />
                <IOPreview title="Output" text={active.output} mode={mode} />
                <IOPreview
                  title="Attributes"
                  text={JSON.stringify(
                    Object.fromEntries(active.attributes.map((a) => [a.key, a.value])),
                  )}
                  mode={mode}
                />
              </>
            ) : (
              <p className="p-8 text-sm text-muted-foreground">
                {busy
                  ? "Loading trace…"
                  : "Select an observation. A deep-linked observation may require loading another page."}
              </p>
            )}
          </main>
        </div>
      </DialogContent>
    </Dialog>
  );
}
