import type { MessageInitShape } from "@bufbuild/protobuf";
import {
  columnSizingFeature,
  createColumnHelper,
  FlexRender,
  tableFeatures,
  useTable,
  type Table,
  type RowData,
} from "@tanstack/react-table";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { mergeObservations, summarizeLoadedSessions } from "./records";
import { ArrowDownIcon } from "lucide-react";
import { traces } from "@/client";
import type {
  Observation,
  ObservationFilterSchema,
  TraceSessionSummary,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { Button } from "@/components/ui/button";
import { ProviderIcon } from "@/components/provider-icon";
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { cost, date, duration, number } from "./format";
import type { TraceSearch } from "./search";

const features = tableFeatures({ columnSizingFeature });
const observation = createColumnHelper<typeof features, Observation>();
const session = createColumnHelper<typeof features, TraceSessionSummary>();
const emptyObservations: Observation[] = [];
const numeric = new Set(["latencySeconds", "totalTokens", "costUsd", "messageCount", "errorCount"]);
function Text({ value, mono = false }: { value: string; mono?: boolean }) {
  return (
    <span className={`block truncate ${mono ? "text-[11px]" : ""}`} title={value}>
      {value || "—"}
    </span>
  );
}
const observationColumns = observation.columns([
  observation.accessor("name", {
    header: "Name",
    size: 240,
    cell: ({ row }) => <Text value={row.original.name} />,
  }),
  observation.accessor("type", {
    header: "Type",
    size: 90,
    cell: ({ row }) => (
      <span
        className={`text-[10px] ${["GENERATION", "TOOL"].includes(row.original.type) ? "font-bold text-foreground" : "text-muted-foreground"}`}
      >
        {row.original.type || "—"}
      </span>
    ),
  }),
  observation.accessor("level", {
    header: "Status",
    size: 75,
    cell: ({ row }) => (
      <span
        className={`inline-flex items-center gap-1.5 text-[10px] ${row.original.level === "ERROR" ? "text-destructive" : "text-muted-foreground"}`}
      >
        <span
          className={`size-1.5 rounded-full ${row.original.level === "ERROR" ? "bg-destructive" : "bg-muted-foreground/40"}`}
        />
        {row.original.level || "DEFAULT"}
      </span>
    ),
  }),
  observation.accessor("startTime", {
    header: "Start time",
    size: 150,
    cell: ({ row }) => (
      <span title={date(row.original.startTime)}>{date(row.original.startTime)}</span>
    ),
  }),
  observation.accessor("latencySeconds", {
    header: "Latency (s)",
    size: 110,
    cell: ({ row }) => (
      <span className={(row.original.latencySeconds ?? 0) > 1 ? "text-destructive" : undefined}>
        {duration(row.original.latencySeconds)}
      </span>
    ),
  }),
  observation.accessor("totalTokens", {
    header: "Tokens",
    size: 75,
    cell: ({ row }) => (
      <span
        title={`${number(row.original.inputTokens)} input · ${number(row.original.outputTokens)} output`}
      >
        {number(row.original.totalTokens)}
      </span>
    ),
  }),
  observation.accessor("costUsd", {
    header: "Cost (USD)",
    size: 105,
    cell: ({ row }) => cost(row.original.costUsd),
  }),
  observation.accessor("model", {
    header: "Model",
    size: 120,
    cell: ({ row }) => <Text value={row.original.model} />,
  }),
  observation.accessor("input", {
    header: "Input",
    size: 240,
    cell: ({ row }) => <Text value={row.original.input} />,
  }),
  observation.accessor("output", {
    header: "Output",
    size: 240,
    cell: ({ row }) => <Text value={row.original.output} />,
  }),
]);
const sessionColumns = session.columns([
  session.accessor("providerName", {
    header: "Provider",
    size: 140,
    cell: ({ row }) => (
      <span className="flex min-w-0 items-center gap-2">
        <ProviderIcon iconUrl={row.original.providerIconUrl} />
        <Text value={row.original.providerName} />
      </span>
    ),
  }),
  session.accessor("id", {
    header: "Session ID",
    size: 260,
    cell: ({ row }) => <Text value={row.original.id} mono />,
  }),
  session.accessor("identities", {
    header: "Identities",
    size: 180,
    cell: ({ row }) => <Text value={row.original.identities.join(", ")} />,
  }),
  session.accessor("lastTurnTime", {
    header: "Last turn",
    size: 150,
    cell: ({ row }) => (
      <span title={date(row.original.lastTurnTime)}>{date(row.original.lastTurnTime)}</span>
    ),
  }),
  session.accessor("messageCount", {
    header: "Messages",
    size: 85,
    cell: ({ row }) => number(row.original.messageCount),
  }),
  session.accessor("errorCount", {
    header: "Errors",
    size: 65,
    cell: ({ row }) => (
      <span className={row.original.errorCount ? "text-destructive" : "text-muted-foreground"}>
        {number(row.original.errorCount)}
      </span>
    ),
  }),
  session.accessor("totalTokens", {
    header: "Tokens",
    size: 90,
    cell: ({ row }) => number(row.original.totalTokens),
  }),
  session.accessor("costUsd", {
    header: "Cost (USD)",
    size: 105,
    cell: ({ row }) => cost(row.original.costUsd),
  }),
]);

/** Cursor requests run only as scrolling demands them. Filter changes remount the
 * stream; failed or canceled requests never replace rows from a different query.
 */
export function TraceGrid({
  agentId,
  filter,
  refresh,
  view,
  ready,
  onInspect,
  onObservations,
}: {
  agentId: string;
  filter: MessageInitShape<typeof ObservationFilterSchema>;
  refresh: boolean;
  view: string;
  ready: boolean;
  onInspect: (patch: TraceSearch) => void;
  onObservations: (rows: Observation[]) => void;
}) {
  const [rows, setRows] = useState<Observation[]>(emptyObservations);
  const [sessionDetails, setSessionDetails] = useState(new Map<string, TraceSessionSummary>());
  const currentRows = useRef<Observation[]>(emptyObservations);
  const [nextCursor, setNextCursor] = useState("");
  const [busy, setBusy] = useState(ready);
  const [error, setError] = useState("");
  const inFlight = useRef(false);
  const request = useRef<AbortController | null>(null);
  const lastCursor = useRef("");
  const received = useRef(new Set<string>());
  const load = useCallback(
    async (cursor: string) => {
      if (inFlight.current || !ready) return;
      inFlight.current = true;
      const abort = new AbortController();
      request.current = abort;
      lastCursor.current = cursor;
      setBusy(true);
      setError("");
      try {
        const page = await traces.listObservations(
          { agentId, filter, cursor, refresh, includeSessionDetails: view === "sessions" },
          { signal: abort.signal },
        );
        if (abort.signal.aborted) return;
        received.current.add(cursor);
        const merged = mergeObservations(currentRows.current, page.observations);
        currentRows.current = merged;
        setRows(merged);
        setSessionDetails((previous) => {
          const next = new Map(previous);
          for (const summary of page.sessions ?? []) {
            const current = next.get(summary.id);
            if (
              !current ||
              (Date.parse(summary.metadataTime) || 0) >= (Date.parse(current.metadataTime) || 0)
            )
              next.set(summary.id, summary);
          }
          return next;
        });
        onObservations(merged);
        if (page.nextCursor && received.current.has(page.nextCursor)) {
          setNextCursor("");
          setError("The trace cursor repeated. Refresh to continue.");
        } else setNextCursor(page.nextCursor);
      } catch (error) {
        if (!abort.signal.aborted)
          setError(error instanceof Error ? error.message : "Unable to load traces");
      } finally {
        if (!abort.signal.aborted) {
          inFlight.current = false;
          setBusy(false);
        }
      }
    },
    [agentId, filter, refresh, ready, onObservations, view],
  );
  useEffect(() => {
    currentRows.current = emptyObservations;
    received.current.clear();
    inFlight.current = false;
    setRows(emptyObservations);
    setSessionDetails(new Map());
    onObservations(emptyObservations);
    setNextCursor("");
    setBusy(ready);
    if (ready) void load("");
    return () => request.current?.abort();
  }, [load, ready, onObservations]);
  const more = useCallback(() => {
    if (nextCursor && !error) void load(nextCursor);
  }, [nextCursor, error, load]);
  // Session grouping is over deduplicated loaded observations. Summing separate
  // page summaries would double-count traces or observations crossing a cursor.
  const sessionRows = useMemo(
    () => summarizeLoadedSessions(rows, sessionDetails),
    [rows, sessionDetails],
  );
  const observations = useTable({
    features,
    columns: observationColumns,
    data: rows,
    getRowId: (row) => `${row.traceId}:${row.id}`,
  });
  const sessions = useTable({
    features,
    columns: sessionColumns,
    data: sessionRows,
    getRowId: (row) => row.id,
  });
  return (
    <>
      {error && (
        <div
          role="alert"
          className="flex shrink-0 items-center gap-3 border-b bg-destructive/5 px-3 py-2 text-xs text-destructive"
        >
          {error}
          <Button size="sm" variant="outline" onClick={() => void load(lastCursor.current)}>
            Retry
          </Button>
        </div>
      )}
      {view === "sessions" ? (
        <Rows
          key="sessions"
          table={sessions}
          label="Trace sessions"
          busy={busy}
          hasNext={!!nextCursor && !error}
          onLoadMore={more}
          empty={ready ? "No sessions match these filters" : "Session browsing is disabled"}
          rowLabel={(row) => `Inspect session ${row.id}`}
          onInspect={(row) =>
            onInspect({ inspectSession: row.id, trace: undefined, observation: undefined })
          }
        />
      ) : (
        <Rows
          key="observations"
          table={observations}
          label="Trace observations"
          busy={busy}
          hasNext={!!nextCursor && !error}
          onLoadMore={more}
          empty={ready ? "No observations match these filters" : "Observation browsing is disabled"}
          rowLabel={(row) => `Inspect ${row.name}`}
          onInspect={(row) =>
            onInspect({ trace: row.traceId, observation: row.id, inspectSession: undefined })
          }
        />
      )}
    </>
  );
}

/** Shared semantic markup for the two concrete table models; no client sorting or grouping. */
function Rows<T extends RowData>({
  table,
  label,
  busy,
  empty,
  rowLabel,
  onInspect,
  hasNext,
  onLoadMore,
}: {
  table: Table<typeof features, T>;
  label: string;
  busy: boolean;
  empty: string;
  rowLabel: (row: T) => string;
  onInspect: (row: T) => void;
  hasNext: boolean;
  onLoadMore: () => void;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const rows = table.getRowModel().rows;
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroll.current,
    getItemKey: (index) => rows[index].id,
    estimateSize: () => 28,
    paddingStart: 32,
    overscan: 8,
    initialRect: { width: 1000, height: 600 },
  });
  const visible = virtualizer.getVirtualItems();
  const top = visible.length ? Math.max(0, visible[0].start - 32) : 0;
  const bottom = visible.length
    ? Math.max(0, virtualizer.getTotalSize() - visible[visible.length - 1].end)
    : 0;
  function nearEnd() {
    const element = scroll.current;
    if (
      element &&
      hasNext &&
      !busy &&
      element.scrollHeight - element.scrollTop - element.clientHeight < 224
    )
      onLoadMore();
  }
  return (
    <div
      ref={scroll}
      className="min-h-0 flex-1 overflow-auto"
      aria-busy={busy}
      role="region"
      aria-label={`${label} scroll area`}
      tabIndex={0}
      onScroll={nearEnd}
      onWheel={(event) => {
        if (event.deltaY > 0) nearEnd();
      }}
    >
      <table
        aria-label={label}
        aria-rowcount={hasNext ? -1 : rows.length + 1}
        className="tracing-table table-fixed border-collapse text-xs tabular-nums"
        style={{ width: table.getTotalSize(), minWidth: "100%" }}
      >
        {label === "Trace sessions" && (
          <caption className="sr-only">
            Messages and identities come from chat records. Trace metrics summarize loaded
            observations.
          </caption>
        )}
        <colgroup>
          {table.getAllLeafColumns().map((column) => (
            <col key={column.id} style={{ width: column.getSize() }} />
          ))}
        </colgroup>
        <TableHeader className="sticky top-0 z-10 bg-muted shadow-[inset_0_-1px_0_var(--border)]">
          {table.getHeaderGroups().map((group) => (
            <TableRow key={group.id} className="hover:bg-transparent">
              {group.headers.map((header) => (
                <TableHead
                  key={header.id}
                  scope="col"
                  aria-sort={
                    ["startTime", "lastTurnTime"].includes(header.column.id)
                      ? "descending"
                      : undefined
                  }
                  className={`h-8 border-r border-border/70 px-2 text-[11px] font-semibold last:border-r-0 ${numeric.has(header.column.id) ? "text-right" : ""}`}
                >
                  <span className="inline-flex items-center gap-1">
                    <FlexRender header={header} />
                    {["startTime", "lastTurnTime"].includes(header.column.id) && (
                      <ArrowDownIcon className="size-3 text-muted-foreground" />
                    )}
                  </span>
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {top > 0 && (
            <tr aria-hidden="true">
              <td
                colSpan={table.getAllLeafColumns().length}
                className="border-0 p-0"
                style={{ height: top }}
              />
            </tr>
          )}
          {visible.map((item) => {
            const row = rows[item.index];
            return (
              <TableRow
                key={row.id}
                aria-rowindex={item.index + 2}
                tabIndex={0}
                aria-label={rowLabel(row.original)}
                className="h-7 cursor-pointer odd:bg-background even:bg-muted/10 hover:bg-accent/60 focus-visible:bg-accent/60 focus-visible:outline focus-visible:-outline-offset-2 focus-visible:outline-ring"
                onClick={() => onInspect(row.original)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onInspect(row.original);
                  }
                }}
              >
                {row.getAllCells().map((cell) => (
                  <TableCell
                    key={cell.id}
                    className={`h-7 overflow-hidden border-r border-border/60 px-2 py-0 text-xs last:border-r-0 ${numeric.has(cell.column.id) ? "text-right" : ""}`}
                  >
                    <FlexRender cell={cell} />
                  </TableCell>
                ))}
              </TableRow>
            );
          })}
          {bottom > 0 && (
            <tr aria-hidden="true">
              <td
                colSpan={table.getAllLeafColumns().length}
                className="border-0 p-0"
                style={{ height: bottom }}
              />
            </tr>
          )}
          {!!rows.length && (busy || hasNext) && (
            <TableRow className="hover:bg-transparent">
              <TableCell
                colSpan={table.getAllLeafColumns().length}
                className="h-7 py-0 text-center text-[11px] text-muted-foreground"
              >
                <span role="status">{busy ? "Loading more…" : "Scroll to load more"}</span>
              </TableCell>
            </TableRow>
          )}
          {!rows.length && (
            <TableRow className="hover:bg-transparent">
              <TableCell
                colSpan={table.getAllLeafColumns().length}
                className="h-28 text-center text-xs text-muted-foreground"
              >
                {busy ? "Loading…" : hasNext ? "Scroll to continue loading" : empty}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </table>
    </div>
  );
}
