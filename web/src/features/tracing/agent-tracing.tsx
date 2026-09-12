import { mergeObservations } from "./records";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { createColumnHelper, FlexRender, tableFeatures, useTable } from "@tanstack/react-table";
import { useForm } from "@trytilde/connection-ui";
import { traces } from "@/client";
import {
  TracingState,
  type Observation,
  type GetTracingStatusResponse,
} from "@/gen/tilde/management/v1/tracing_pb.js";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectTrigger,
  SelectContent,
  SelectItem,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ExternalLink } from "./links";
import { Inspector } from "./inspector";
import { cost, date, duration, number } from "./format";
import { ranges, parseSearch, type TraceSearch } from "./search";
import { RefreshCwIcon, SlidersHorizontalIcon, ActivityIcon } from "lucide-react";

const features = tableFeatures({});
const helper = createColumnHelper<typeof features, Observation>();
const fieldNames = {
  name: "Name",
  type: "Type",
  level: "Status",
  startTime: "Start time",
  latencySeconds: "Latency",
  timeToFirstTokenSeconds: "Time to first token",
  input: "Input",
  output: "Output",
  model: "Model",
  totalTokens: "Tokens",
  costUsd: "Cost",
  sessionId: "Session ID",
  traceId: "Trace ID",
  userId: "User",
  environment: "Environment",
  tags: "Tags",
};
type Column = keyof typeof fieldNames;
const defaultColumns = Object.keys(fieldNames).filter(
  (k) => !["userId", "environment", "tags"].includes(k),
) as Column[];
const presets = { all: "All", llm: "LLM Calls", tool: "Tool Calls", errors: "Errors" };
type FilterValues = {
  name: string;
  model: string;
  session: string;
  invocation: string;
  input: string;
  output: string;
  type: string;
  level: string;
  from: string;
  to: string;
};
const filterFields: Array<[keyof FilterValues, string]> = [
  ["name", "Name contains"],
  ["model", "Model"],
  ["session", "Session ID"],
  ["invocation", "Invocation ID"],
  ["input", "Input search"],
  ["output", "Output search"],
  ["type", "Observation type"],
  ["level", "Status level"],
];
const rangeLabels: Record<string, string> = {
  "15m": "Last 15 minutes",
  "1h": "Last hour",
  "6h": "Last 6 hours",
  "24h": "Last 24 hours",
  "7d": "Last 7 days",
  "30d": "Last 30 days",
  custom: "Custom range",
};
function localDate(value?: string) {
  if (!value) return "";
  const date = new Date(value);
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
}
export function AgentTracing({ agentId }: { agentId: string }) {
  const search = parseSearch(useSearch({ strict: false }) as Record<string, unknown>);
  const navigate = useNavigate();
  function update(patch: TraceSearch) {
    void navigate({ to: ".", search: (previous) => ({ ...previous, ...patch }), replace: true });
  }
  const generation = useRef(0);
  const [status, setStatus] = useState<GetTracingStatusResponse>();
  const [rows, setRows] = useState<Observation[]>([]);
  const [cursor, setCursor] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [columnsOpen, setColumnsOpen] = useState(false);
  const [visible, setVisible] = useState<Column[]>(() => {
    try {
      const stored = JSON.parse(localStorage.getItem("tilde.tracing.columns") || "null");
      return Array.isArray(stored) ? stored.filter((k) => k in fieldNames) : defaultColumns;
    } catch {
      return defaultColumns;
    }
  });
  const form = useForm<FilterValues>({
    defaultValues: Object.fromEntries([
      ...filterFields.map(([key]) => [key, search[key] ?? ""]),
      ["from", search.from ?? ""],
      ["to", search.to ?? ""],
    ]),
  });
  const queryKey = JSON.stringify([
    search.range,
    search.from,
    search.to,
    search.preset,
    search.name,
    search.model,
    search.session,
    search.invocation,
    search.input,
    search.output,
    search.type,
    search.level,
  ]);
  useEffect(() => {
    form.reset(
      Object.fromEntries([
        ...filterFields.map(([key]) => [key, search[key] ?? ""]),
        ["from", localDate(search.from)],
        ["to", localDate(search.to)],
      ]),
    );
  }, [queryKey]);
  const filter = useMemo(() => {
    const end = new Date();
    const milliseconds = ranges[(search.range ?? "24h") as keyof typeof ranges] ?? ranges["24h"];
    return {
      fromTime:
        search.range === "custom" && search.from
          ? new Date(search.from).toISOString()
          : new Date(end.getTime() - milliseconds).toISOString(),
      toTime:
        search.range === "custom" && search.to
          ? new Date(search.to).toISOString()
          : end.toISOString(),
      name: search.name,
      model: search.model,
      sessionId: search.session,
      invocationId: search.invocation,
      inputSearch: search.input,
      outputSearch: search.output,
      type:
        search.preset === "llm" ? "GENERATION" : search.preset === "tool" ? "TOOL" : search.type,
      level: search.preset === "errors" ? "ERROR" : search.level,
    };
  }, [queryKey, attempt]);
  useEffect(() => {
    const abort = new AbortController();
    setError("");
    setStatus(undefined);
    void traces
      .getTracingStatus({ agentId }, { signal: abort.signal })
      .then(setStatus)
      .catch((e) => {
        if (!abort.signal.aborted) setError(e.message);
      });
    return () => abort.abort();
  }, [agentId, attempt]);
  const ready = status?.state === TracingState.READY;
  const disabled = status?.state === TracingState.DISABLED;
  useEffect(() => {
    if (!ready) {
      setRows([]);
      setCursor("");
      return;
    }
    generation.current++;
    const abort = new AbortController();
    setBusy(true);
    setError("");
    setRows([]);
    setCursor("");
    void traces
      .listObservations({ agentId, filter, refresh: attempt > 0 }, { signal: abort.signal })
      .then((page) => {
        setRows(mergeObservations([], page.observations));
        setCursor(page.nextCursor);
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(e.message);
      })
      .finally(() => {
        if (!abort.signal.aborted) setBusy(false);
      });
    return () => abort.abort();
  }, [agentId, ready, filter]);
  async function more() {
    const expected = generation.current;
    setBusy(true);
    setError("");
    try {
      const page = await traces.listObservations({ agentId, filter, cursor });
      if (generation.current !== expected) return;
      setRows((old) => mergeObservations(old, page.observations));
      setCursor(page.nextCursor);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Unable to load observations");
    } finally {
      if (generation.current === expected) setBusy(false);
    }
  }
  const columns = useMemo(
    () =>
      helper.columns(
        visible.map((key) =>
          helper.accessor(key, {
            header: fieldNames[key],
            cell: ({ row }) => {
              const r = row.original;
              if (key === "startTime")
                return <span className="whitespace-nowrap">{date(r.startTime)}</span>;
              if (key === "latencySeconds" || key === "timeToFirstTokenSeconds")
                return duration(r[key]);
              if (key === "costUsd") return cost(r.costUsd);
              if (key === "totalTokens")
                return (
                  <span title={`${number(r.inputTokens)} input · ${number(r.outputTokens)} output`}>
                    {number(r.totalTokens)}
                  </span>
                );
              if (key === "level")
                return (
                  <span
                    className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${r.level === "ERROR" ? "bg-destructive/10 text-destructive" : "bg-muted text-muted-foreground"}`}
                  >
                    {r.level || "DEFAULT"}
                  </span>
                );
              if (key === "type")
                return <span className="rounded border px-1.5 py-0.5 text-[10px]">{r.type}</span>;
              const text = key === "tags" ? r.tags.join(", ") : String(r[key] ?? "");
              return (
                <span
                  className={`block max-w-64 truncate ${key.endsWith("Id") ? "font-mono text-[10px]" : ""}`}
                  title={text}
                >
                  {text || "—"}
                </span>
              );
            },
          }),
        ),
      ),
    [visible],
  );
  const table = useTable({
    features,
    columns,
    data: rows,
    getRowId: (r) => `${r.traceId}:${r.id}`,
  });
  const sessions = useMemo(() => {
    const result = new Map<string, Observation[]>();
    for (const row of rows)
      if (row.sessionId) result.set(row.sessionId, [...(result.get(row.sessionId) ?? []), row]);
    return [...result.entries()];
  }, [rows]);
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  return (
    <section className="space-y-4 py-6" aria-label="Agent tracing">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="flex items-center gap-2 text-lg font-semibold">
            <ActivityIcon className="size-4 text-muted-foreground" />
            Tracing
          </h2>
          <p className="mt-1 text-xs text-muted-foreground">
            Observations and sessions from Langfuse
          </p>
        </div>
        <div className="flex items-center gap-2">
          <ExternalLink href={status?.projectUrl} label="Open project in Langfuse" />
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setAttempt((n) => n + 1)}
            disabled={busy}
          >
            <RefreshCwIcon className={busy ? "animate-spin" : ""} />
            Refresh
          </Button>
        </div>
      </header>
      {(!ready || error) && (
        <div
          role={error || status?.state === TracingState.UNAVAILABLE ? "alert" : "status"}
          className="rounded-lg border bg-muted/30 p-4 text-sm text-muted-foreground"
        >
          {error || status?.message || "Checking tracing availability…"}
          {disabled && (
            <p className="mt-1 text-xs">
              Your administrator can enable Langfuse for this installation. Trace uploads are
              discarded while it is disabled.
            </p>
          )}
        </div>
      )}
      <fieldset disabled={!ready} className="space-y-3 disabled:opacity-50">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Tabs
            value={search.view ?? "observations"}
            onValueChange={(value) => update({ view: String(value) })}
          >
            <TabsList aria-label="Tracing views">
              <TabsTrigger value="observations" disabled={!ready}>
                Observations
              </TabsTrigger>
              <TabsTrigger value="sessions" disabled={!ready}>
                Sessions
              </TabsTrigger>
            </TabsList>
          </Tabs>
          <div className="flex items-center gap-2">
            <Label htmlFor={`trace-range-${agentId}`} className="sr-only">
              Date range
            </Label>
            <Select
              value={search.range ?? "24h"}
              disabled={!ready}
              onValueChange={(value) => {
                if (value) {
                  update({ range: value });
                  if (value === "custom") setFiltersOpen(true);
                }
              }}
            >
              <SelectTrigger id={`trace-range-${agentId}`} size="sm" aria-label="Date range">
                <SelectValue>{rangeLabels[search.range ?? "24h"]}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                {Object.entries(rangeLabels).map(([value, label]) => (
                  <SelectItem key={value} value={value}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setFiltersOpen(!filtersOpen)}
            >
              <SlidersHorizontalIcon />
              Filters
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setColumnsOpen(!columnsOpen)}
            >
              Columns
            </Button>
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex gap-1">
            {Object.entries(presets).map(([id, label]) => (
              <Button
                type="button"
                key={id}
                variant={(search.preset ?? "all") === id ? "secondary" : "ghost"}
                size="sm"
                onClick={() => update({ preset: id })}
              >
                {label}
              </Button>
            ))}
          </div>
          <span className="text-[11px] text-muted-foreground">Newest first · {timezone}</span>
        </div>
        {filtersOpen && (
          <div className="grid gap-3 rounded-lg border bg-muted/20 p-4 sm:grid-cols-2 lg:grid-cols-4">
            {filterFields.map(([key, label]) => (
              <div className="space-y-1" key={key}>
                <Label htmlFor={`tracing-${key}`}>{label}</Label>
                <Input id={`tracing-${key}`} {...form.register(key)} />
              </div>
            ))}
            {search.range === "custom" &&
              ["from", "to"].map((key) => (
                <div key={key} className="space-y-1">
                  <Label htmlFor={`tracing-${key}`}>
                    {key === "from" ? "From" : "Until"} ({timezone})
                  </Label>
                  <Input
                    id={`tracing-${key}`}
                    type="datetime-local"
                    {...form.register(key as "from" | "to", { required: true })}
                  />
                </div>
              ))}
            <div className="flex items-end gap-2">
              <Button
                type="button"
                onClick={form.handleSubmit(
                  (values) => {
                    if (
                      search.range === "custom" &&
                      (!values.from ||
                        !values.to ||
                        Date.parse(values.from) >= Date.parse(values.to))
                    ) {
                      setError("Choose a valid start and end time");
                      return;
                    }
                    update({
                      ...values,
                      type: values.type.toUpperCase(),
                      level: values.level.toUpperCase(),
                      preset: values.type || values.level ? "all" : search.preset,
                      from: values.from ? new Date(values.from).toISOString() : undefined,
                      to: values.to ? new Date(values.to).toISOString() : undefined,
                    });
                    setFiltersOpen(false);
                  },
                  () => setError("Choose both a start and end time"),
                )}
              >
                Apply
              </Button>
              <Button
                type="button"
                variant="ghost"
                onClick={() => {
                  update({
                    ...Object.fromEntries(filterFields.map(([key]) => [key, undefined])),
                    from: undefined,
                    to: undefined,
                    range: search.range === "custom" ? "24h" : search.range,
                  });
                }}
              >
                Clear
              </Button>
            </div>
          </div>
        )}
        {columnsOpen && (
          <div className="flex flex-wrap gap-3 rounded-lg border p-3">
            {Object.entries(fieldNames).map(([key, label]) => (
              <label key={key} className="flex items-center gap-1.5 text-xs">
                <Switch
                  checked={visible.includes(key as Column)}
                  onCheckedChange={(checked) => {
                    const next = checked
                      ? [...visible, key as Column]
                      : visible.filter((v) => v !== key);
                    setVisible(next);
                    localStorage.setItem("tilde.tracing.columns", JSON.stringify(next));
                  }}
                />
                {label}
              </label>
            ))}
          </div>
        )}
      </fieldset>
      <div className="overflow-x-auto rounded-lg border" aria-busy={busy}>
        {search.view === "sessions" ? (
          <Table aria-label="Trace sessions">
            <TableHeader>
              <TableRow>
                {[
                  "Session ID",
                  "Loaded observations",
                  "Loaded traces",
                  "Latest observation",
                  "",
                ].map((title) => (
                  <TableHead key={title}>{title}</TableHead>
                ))}
              </TableRow>
            </TableHeader>
            <TableBody>
              {sessions.map(([id, items]) => (
                <TableRow key={id}>
                  <TableCell>
                    <Button
                      type="button"
                      variant="link"
                      onClick={() =>
                        update({ inspectSession: id, trace: undefined, observation: undefined })
                      }
                    >
                      {id}
                    </Button>
                  </TableCell>
                  <TableCell>{items.length}</TableCell>
                  <TableCell>{new Set(items.map((r) => r.traceId)).size}</TableCell>
                  <TableCell>{date(items[0].startTime)}</TableCell>
                  <TableCell>
                    <ExternalLink href={items[0].sessionUrl} label="Open session in Langfuse" />
                  </TableCell>
                </TableRow>
              ))}
              {!sessions.length && (
                <TableRow>
                  <TableCell colSpan={5} className="h-40 text-center text-muted-foreground">
                    {busy
                      ? "Loading sessions…"
                      : ready
                        ? "No recorded sessions in these results"
                        : "Session browsing is disabled"}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        ) : (
          <Table aria-label="Trace observations">
            <TableHeader className="bg-muted/40">
              {table.getHeaderGroups().map((group) => (
                <TableRow key={group.id}>
                  {group.headers.map((header) => (
                    <TableHead key={header.id} className="whitespace-nowrap px-3 text-xs">
                      <FlexRender header={header} />
                    </TableHead>
                  ))}
                </TableRow>
              ))}
            </TableHeader>
            <TableBody>
              {table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  tabIndex={0}
                  className="cursor-pointer"
                  aria-label={`Inspect ${row.original.name}`}
                  onClick={() =>
                    update({
                      trace: row.original.traceId,
                      observation: row.original.id,
                      inspectSession: undefined,
                    })
                  }
                  onKeyDown={(e) => {
                    if (e.key === "Enter")
                      update({
                        trace: row.original.traceId,
                        observation: row.original.id,
                        inspectSession: undefined,
                      });
                  }}
                >
                  {row.getAllCells().map((cell) => (
                    <TableCell key={cell.id} className="h-12 px-3 text-xs">
                      <FlexRender cell={cell} />
                    </TableCell>
                  ))}
                </TableRow>
              ))}
              {!rows.length && (
                <TableRow>
                  <TableCell
                    colSpan={Math.max(1, visible.length)}
                    className="h-40 text-center text-muted-foreground"
                  >
                    {busy
                      ? "Loading observations…"
                      : ready
                        ? "No observations match these filters"
                        : "Observation browsing is disabled"}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        )}
      </div>
      <footer className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
        <p>
          {rows.length} observations loaded
          {search.view === "sessions" ? " · Session groups reflect loaded observations only" : ""}
          {cursor ? " · Partial results" : ""}
        </p>
        {cursor && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() => void more()}
          >
            Load more
          </Button>
        )}
      </footer>
      {ready && (search.trace || search.inspectSession) && (
        <Inspector
          key={search.trace || search.inspectSession}
          agentId={agentId}
          traceId={search.trace}
          sessionId={search.inspectSession}
          selectedId={search.observation}
          filter={filter}
          onClose={() =>
            update({ trace: undefined, observation: undefined, inspectSession: undefined })
          }
        />
      )}
    </section>
  );
}
