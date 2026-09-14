import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearch, Link } from "@tanstack/react-router";
import { createColumnHelper, FlexRender, tableFeatures, useTable } from "@tanstack/react-table";
import { useForm } from "@trytilde/connection-ui";
import { logs } from "@/client";
import {
  LogsState,
  type GetLogsStatusResponse,
  type LogRecord,
} from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import {
  Table,
  TableHeader,
  TableHead,
  TableBody,
  TableRow,
  TableCell,
} from "@/components/ui/table";
import { Dialog, DialogContent, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { IOPreview } from "../tracing/io-preview";
import { ranges } from "../tracing/search";
import { parseLogSearch, type LogSearch } from "./search";
const features = tableFeatures({});
const helper = createColumnHelper<typeof features, LogRecord>();
const labels = {
  timestamp: "Time",
  severity: "Severity",
  body: "Message",
  service: "Service",
  invocationId: "Invocation",
  traceId: "Trace ID",
  spanId: "Span ID",
};
type Column = keyof typeof labels;
const levels: Record<string, string> = {
  "0": "All",
  "5": "Debug and above",
  "9": "Info and above",
  "13": "Warnings and above",
  "17": "Errors and above",
};
const defaults: Column[] = ["timestamp", "severity", "body", "service", "invocationId"];
function localTime(value?: string) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isFinite(date.getTime())
    ? new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16)
    : "";
}
export function AgentLogs({ agentId }: { agentId: string }) {
  const search = parseLogSearch(useSearch({ strict: false }) as Record<string, unknown>);
  const navigate = useNavigate();
  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState<GetLogsStatusResponse>();
  const [records, setRecords] = useState<LogRecord[]>([]);
  const [cursor, setCursor] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [live, setLive] = useState(false);
  const [selected, setSelected] = useState<LogRecord>();
  const [columnsOpen, setColumnsOpen] = useState(false);
  const [visible, setVisible] = useState<Column[]>(() => {
    try {
      const saved: unknown = JSON.parse(localStorage.getItem("tilde.logs.columns") ?? "null");
      return Array.isArray(saved) && saved.some((v) => v in labels)
        ? saved.filter((v) => v in labels)
        : defaults;
    } catch {
      return defaults;
    }
  });
  const key = JSON.stringify(search);
  const form = useForm<LogSearch>({
    defaultValues: { ...search, from: localTime(search.from), to: localTime(search.to) },
  });
  useEffect(() => {
    form.reset({ ...search, from: localTime(search.from), to: localTime(search.to) });
  }, [key]);
  const generation = useRef(0);
  const filter = useMemo(() => {
    const range = ranges[(search.range ?? "24h") as keyof typeof ranges] ?? ranges["24h"];
    const to = search.range === "custom" && search.to ? new Date(search.to) : new Date();
    const from =
      search.range === "custom" && search.from
        ? new Date(search.from)
        : new Date(to.getTime() - range);
    return {
      fromTime: Number.isFinite(from.getTime()) ? from.toISOString() : "",
      toTime: Number.isFinite(to.getTime()) ? to.toISOString() : "",
      search: search.text ?? "",
      minimumSeverity: Number(search.severity ?? 0),
      invocationId: search.invocation ?? "",
      traceId: search.trace ?? "",
      service: search.service ?? "",
    };
  }, [key, attempt]);
  useEffect(() => {
    const abort = new AbortController();
    void logs
      .getLogsStatus({ agentId }, { signal: abort.signal })
      .then((s) => {
        if (!abort.signal.aborted) setStatus(s);
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(e.message);
      });
    return () => abort.abort();
  }, [agentId, attempt]);
  const ready = status?.state === LogsState.READY;
  useEffect(() => {
    generation.current++;
    if (!ready) {
      setRecords([]);
      return;
    }
    const abort = new AbortController();
    setBusy(true);
    setError("");
    setCursor("");
    setSelected(undefined);
    void logs
      .listLogs({ agentId, filter }, { signal: abort.signal })
      .then((page) => {
        if (abort.signal.aborted) return;
        setRecords(page.records);
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
  useEffect(() => {
    if (!live || !ready || busy || error) return;
    const timer = setTimeout(() => {
      if (!document.hidden) setAttempt((a) => a + 1);
      else setLive(false);
    }, 3000);
    return () => clearTimeout(timer);
  }, [live, ready, busy, error, attempt]);
  async function more() {
    const expected = generation.current;
    setBusy(true);
    setError("");
    try {
      const page = await logs.listLogs({ agentId, filter, cursor });
      if (expected !== generation.current) return;
      setRecords((old) =>
        [...new Map([...old, ...page.records].map((r) => [r.id, r])).values()].slice(0, 1000),
      );
      setCursor(page.nextCursor);
    } catch (e) {
      if (expected === generation.current)
        setError(e instanceof Error ? e.message : "Log query failed");
    } finally {
      if (expected === generation.current) setBusy(false);
    }
  }
  const columns = useMemo(
    () =>
      helper.columns(
        visible.map((key) =>
          helper.accessor(key, {
            header: labels[key],
            cell: ({ row }) =>
              key === "timestamp" ? (
                new Date(row.original.timestamp).toLocaleString()
              ) : (
                <span
                  className={
                    key === "body"
                      ? "block max-w-xl truncate font-mono text-xs"
                      : "block max-w-48 truncate"
                  }
                >
                  {row.original[key] || "—"}
                </span>
              ),
          }),
        ),
      ),
    [visible],
  );
  const table = useTable({ columns, data: records, features });
  function update(values: LogSearch) {
    setLive(false);
    void navigate({ to: "/agent/$agentId/logs", params: { agentId }, search: values });
  }
  return (
    <section className="space-y-4" aria-label="Agent logs">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Logs</h2>
          <p className="text-xs text-muted-foreground">
            {Intl.DateTimeFormat().resolvedOptions().timeZone} · newest first
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            disabled={!ready || search.range === "custom"}
            onClick={() => setLive((v) => !v)}
          >
            {live ? "Pause live" : "Live"}
          </Button>
          <Button variant="outline" disabled={busy} onClick={() => setAttempt((a) => a + 1)}>
            Refresh
          </Button>
        </div>
      </div>
      {status?.state === LogsState.DISABLED && (
        <p role="status" className="rounded-lg border p-4 text-muted-foreground">
          {status.message}
        </p>
      )}
      {status?.state === LogsState.UNAVAILABLE && (
        <p role="alert" className="rounded-lg border p-4">
          {status.message}
        </p>
      )}
      {!status && !error && <p role="status">Loading log configuration…</p>}
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
      <fieldset disabled={!ready} className="space-y-3 disabled:opacity-50">
        <div className="flex flex-wrap items-end gap-3">
          <div className="space-y-1">
            <Label>Time range</Label>
            <Select
              value={search.range ?? "24h"}
              onValueChange={(value) => update({ ...search, range: value ?? "24h" })}
            >
              <SelectTrigger aria-label="Log time range" className="w-40">
                <SelectValue>
                  {search.range === "custom" ? "Custom" : `Last ${search.range ?? "24h"}`}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {[...Object.keys(ranges), "custom"].map((v) => (
                  <SelectItem key={v} value={v}>
                    {v === "custom" ? "Custom" : `Last ${v}`}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label>Severity</Label>
            <Select
              value={search.severity ?? "0"}
              onValueChange={(value) => update({ ...search, severity: value ?? "0" })}
            >
              <SelectTrigger aria-label="Log severity" className="w-40">
                <SelectValue>{levels[search.severity ?? "0"] ?? "All"}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                {Object.entries(levels).map(([v, label]) => (
                  <SelectItem key={v} value={v}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <Button variant="outline" onClick={() => setColumnsOpen((v) => !v)}>
            Columns
          </Button>
        </div>
        <form
          className="flex flex-wrap items-end gap-3"
          onSubmit={form.handleSubmit((values) =>
            update({
              ...search,
              ...values,
              from: values.from ? new Date(values.from).toISOString() : undefined,
              to: values.to ? new Date(values.to).toISOString() : undefined,
            }),
          )}
        >
          {[
            ["text", "Search message"],
            ["service", "Service"],
            ["invocation", "Invocation ID"],
            ["trace", "Trace ID"],
            ...(search.range === "custom"
              ? [
                  ["from", "From"],
                  ["to", "To"],
                ]
              : []),
          ].map(([key, label]) => (
            <div className="space-y-1" key={key}>
              <Label htmlFor={`logs-${key}`}>{label}</Label>
              <Input
                id={`logs-${key}`}
                type={key === "from" || key === "to" ? "datetime-local" : "text"}
                required={key === "from" || key === "to"}
                {...form.register(key as keyof LogSearch)}
              />
            </div>
          ))}
          <Button type="submit">Apply</Button>
          <Button type="button" variant="outline" onClick={() => update({})}>
            Clear
          </Button>
        </form>
        {columnsOpen && (
          <div className="flex flex-wrap gap-3 rounded-lg border p-3">
            {Object.entries(labels).map(([key, label]) => (
              <label key={key} className="flex items-center gap-2 text-xs">
                <Switch
                  checked={visible.includes(key as Column)}
                  onCheckedChange={(checked) => {
                    const next = checked
                      ? [...visible, key as Column]
                      : visible.filter((v) => v !== key);
                    setVisible(next);
                    localStorage.setItem("tilde.logs.columns", JSON.stringify(next));
                  }}
                />
                {label}
              </label>
            ))}
          </div>
        )}
      </fieldset>
      {live && (
        <p role="status" className="text-xs text-muted-foreground">
          Live refresh shows the latest 100 records every three seconds. High-volume intervals may
          be skipped; pause and use a fixed time range to browse history.
        </p>
      )}
      {ready && (
        <>
          <div className="overflow-x-auto rounded-lg border" aria-busy={busy}>
            <Table aria-label="Agent log records">
              <TableHeader>
                {table.getHeaderGroups().map((group) => (
                  <TableRow key={group.id}>
                    {group.headers.map((header) => (
                      <TableHead key={header.id}>
                        <FlexRender header={header} />
                      </TableHead>
                    ))}
                  </TableRow>
                ))}
              </TableHeader>
              <TableBody>
                {table.getRowModel().rows.map((row) => (
                  <TableRow
                    key={row.original.id}
                    tabIndex={0}
                    aria-label={`Inspect log ${row.original.id}`}
                    className="cursor-pointer"
                    onClick={() => {
                      setLive(false);
                      setSelected(row.original);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        setLive(false);
                        setSelected(row.original);
                      }
                    }}
                  >
                    {row.getAllCells().map((cell) => (
                      <TableCell key={cell.id}>
                        <FlexRender cell={cell} />
                      </TableCell>
                    ))}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            {!busy && !records.length && !error && (
              <p className="p-6 text-muted-foreground">No logs match these filters.</p>
            )}
          </div>
          <div className="flex items-center justify-between">
            <p className="text-xs text-muted-foreground">
              {records.length} records loaded
              {records.length >= 1000
                ? " · display limit reached; narrow the filters"
                : cursor
                  ? " · more available"
                  : ""}
            </p>
            {cursor && !live && records.length < 1000 && (
              <Button variant="outline" disabled={busy} onClick={() => void more()}>
                Load more
              </Button>
            )}
          </div>
        </>
      )}
      <Dialog
        open={!!selected}
        onOpenChange={(open) => {
          if (!open) setSelected(undefined);
        }}
      >
        <DialogContent className="max-h-[85vh] overflow-auto sm:max-w-4xl">
          <DialogTitle>Log details</DialogTitle>
          <DialogDescription>
            {selected?.timestamp} · {selected?.severity || "Unspecified severity"}
          </DialogDescription>
          {selected && (
            <div className="space-y-4">
              <div className="flex flex-wrap gap-4 text-xs">
                <span>Service: {selected.service || "—"}</span>
                <span>Invocation: {selected.invocationId}</span>
                <span>Span: {selected.spanId || "—"}</span>
                {selected.traceId && (
                  <Link
                    className="underline"
                    to="/agent/$agentId/tracing"
                    params={{ agentId }}
                    search={{ trace: selected.traceId }}
                  >
                    View trace
                  </Link>
                )}
              </div>
              <IOPreview title="Message" text={selected.body} mode="pretty" />
              {(
                [
                  ["Attributes", selected.attributes],
                  ["Resource", selected.resourceAttributes],
                  ["Scope", selected.scopeAttributes],
                ] as const
              ).map(([label, attrs]) => (
                <div key={label}>
                  <h3 className="mb-2 font-medium">{label}</h3>
                  <pre className="max-h-72 overflow-auto rounded-md bg-muted p-3 text-xs whitespace-pre-wrap break-all">
                    {JSON.stringify(attrs, null, 2)}
                  </pre>
                </div>
              ))}
            </div>
          )}
        </DialogContent>
      </Dialog>
    </section>
  );
}
