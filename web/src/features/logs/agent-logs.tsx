import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearch, Link } from "@tanstack/react-router";
import { Controller, useForm } from "@trytilde/connection-ui";
import { logs } from "@/client";
import { type LogRecord } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
import { Button } from "@/components/ui/button";
import { TimeRangeSelect } from "../observability/time-range-select";
import { Sheet, SheetContent, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ArrowRightIcon, RefreshCwIcon, XIcon, PlayIcon, PauseIcon } from "lucide-react";
import { ActivityChart } from "../observability/activity-chart";
import { FilterSearchInput } from "../observability/filter-search-input";
import { TraceDateTimePicker } from "../tracing/date-time-picker";
import { TracePayload } from "../tracing/trace-payload";
import { date } from "../tracing/format";
import { ranges } from "../tracing/search";
import { parseLogSearch, type LogSearch } from "./search";
import { logSuggestions, parseLogQuery, serializeLogQuery } from "./filter-query";
import { LogTable } from "./log-table";

const levels = { "0": "All", "5": "Debug", "9": "Info", "13": "Warnings", "17": "Errors" };
function localTime(value: string) {
  const d = new Date(value);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 19);
}
export function AgentLogs({ agentId }: { agentId: string }) {
  const search = parseLogSearch(useSearch({ strict: false }) as Record<string, unknown>);
  const navigate = useNavigate();
  const [attempt, setAttempt] = useState(0),
    [live, setLive] = useState(false);
  const [records, setRecords] = useState<LogRecord[]>([]),
    [cursor, setCursor] = useState("");
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const [selected, setSelected] = useState<LogRecord>();
  const [activityWindow, setActivityWindow] = useState<{
    fromTime: string;
    toTime: string;
    previous: Pick<LogSearch, "range" | "from" | "to">;
  } | null>(null);
  const generation = useRef(0),
    inFlight = useRef(false),
    pageRequest = useRef<AbortController | null>(null);
  const seenCursors = useRef(new Set<string>());
  const key = JSON.stringify(search);
  function update(patch: LogSearch, preserveActivity = false) {
    setLive(false);
    setError("");
    if (!preserveActivity && ("range" in patch || "from" in patch || "to" in patch))
      setActivityWindow(null);
    void navigate({
      to: "/agent/$agentId/logs",
      params: { agentId },
      search: (previous) => ({ ...previous, ...patch }),
      replace: true,
    });
  }
  const filter = useMemo(() => {
    const end = new Date(),
      span = ranges[(search.range ?? "24h") as keyof typeof ranges] ?? ranges["24h"];
    return {
      fromTime:
        search.range === "custom" && search.from
          ? new Date(search.from).toISOString()
          : (activityWindow?.fromTime ?? new Date(end.getTime() - span).toISOString()),
      toTime:
        search.range === "custom" && search.to
          ? new Date(search.to).toISOString()
          : (activityWindow?.toTime ?? end.toISOString()),
      search: search.text ?? "",
      minimumSeverity: Number(search.severity ?? 0),
      invocationId: search.invocation ?? "",
      threadId: search.session ?? "",
      runId: search.run ?? "",
      scope: search.scope ?? "",
      traceId: search.trace ?? "",
      service: search.service ?? "",
    };
  }, [key, attempt, activityWindow]);
  const activityKey = JSON.stringify({
    ...filter,
    fromTime: activityWindow?.fromTime ?? filter.fromTime,
    toTime: activityWindow?.toTime ?? filter.toTime,
  });
  const activityFilter = useMemo(
    () => ({
      ...filter,
      fromTime: activityWindow?.fromTime ?? filter.fromTime,
      toTime: activityWindow?.toTime ?? filter.toTime,
    }),
    [activityKey],
  );
  const selectedRange =
    activityWindow &&
    (filter.fromTime !== activityWindow.fromTime || filter.toTime !== activityWindow.toTime)
      ? { from: filter.fromTime, to: filter.toTime }
      : null;
  const loadMetrics = useCallback(
    async (signal: AbortSignal) => {
      const response = await logs.getLogMetrics({ agentId, filter: activityFilter }, { signal });
      return response.buckets.map((bucket) => ({ ...bucket, count: bucket.logCount }));
    },
    [agentId, activityFilter, attempt],
  );
  const suggest = useCallback(
    (query: string, caret: number) => logSuggestions(query, caret, records),
    [records],
  );
  const form = useForm<{ from: string; to: string }>({
    defaultValues: { from: localTime(filter.fromTime), to: localTime(filter.toTime) },
  });
  useEffect(() => {
    form.reset({ from: localTime(filter.fromTime), to: localTime(filter.toTime) });
  }, [filter.fromTime, filter.toTime]);
  function applyRange() {
    const { from, to } = form.getValues();
    if (
      !Number.isFinite(Date.parse(from)) ||
      !Number.isFinite(Date.parse(to)) ||
      Date.parse(from) >= Date.parse(to)
    ) {
      form.setError("root", { message: "Choose an end time after the start time." });
      return;
    }
    if (Date.parse(to) - Date.parse(from) > 30 * 86400_000) {
      form.setError("root", { message: "Use a time range of at most 30 days." });
      return;
    }
    form.clearErrors();
    update({ from: new Date(from).toISOString(), to: new Date(to).toISOString() });
  }
  useEffect(() => {
    generation.current++;
    pageRequest.current?.abort();
    seenCursors.current.clear();
    setRecords([]);
    setCursor("");
    setSelected(undefined);
    const abort = new AbortController();
    inFlight.current = true;
    setBusy(true);
    setError("");
    void logs
      .listLogs({ agentId, filter }, { signal: abort.signal })
      .then((page) => {
        if (!abort.signal.aborted) {
          setRecords([...new Map(page.records.map((r) => [r.id, r])).values()]);
          setCursor(page.nextCursor);
        }
      })
      .catch((e) => {
        if (!abort.signal.aborted) setError(e.message);
      })
      .finally(() => {
        if (!abort.signal.aborted) {
          inFlight.current = false;
          setBusy(false);
        }
      });
    return () => {
      abort.abort();
      pageRequest.current?.abort();
    };
  }, [agentId, filter]);
  useEffect(() => {
    if (!live || busy || error) return;
    const timer = setTimeout(() => {
      if (!document.hidden) setAttempt((a) => a + 1);
      else setLive(false);
    }, 3000);
    return () => clearTimeout(timer);
  }, [live, busy, error, attempt]);
  async function more() {
    if (inFlight.current || !cursor || live || seenCursors.current.has(cursor)) return;
    const expected = generation.current,
      used = cursor,
      abort = new AbortController();
    pageRequest.current = abort;
    inFlight.current = true;
    setBusy(true);
    setError("");
    try {
      const page = await logs.listLogs({ agentId, filter, cursor }, { signal: abort.signal });
      if (abort.signal.aborted || expected !== generation.current) return;
      seenCursors.current.add(used);
      setRecords((old) => [...new Map([...old, ...page.records].map((r) => [r.id, r])).values()]);
      setCursor(page.nextCursor === used ? "" : page.nextCursor);
    } catch (e) {
      if (!abort.signal.aborted && expected === generation.current)
        setError(e instanceof Error ? e.message : "Log query failed");
    } finally {
      if (expected === generation.current) {
        inFlight.current = false;
        setBusy(false);
      }
    }
  }
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  return (
    <section className="flex min-h-0 flex-1 flex-col" aria-label="Agent logs">
      <div className="trace-controls relative z-20 shrink-0 border-b bg-background">
        <div
          role="group"
          aria-label="Log filter bar"
          className="flex min-h-10 w-full flex-wrap items-center gap-2 px-3 py-1.5"
        >
          <fieldset className="flex min-w-0 flex-1 flex-wrap items-center gap-2 disabled:opacity-60">
            <Tooltip>
              <TooltipTrigger render={<div className="shrink-0" />}>
                <Tabs
                  value={search.severity ?? "0"}
                  onValueChange={(value) => update({ severity: String(value) })}
                  className="gap-0"
                >
                  <TabsList aria-label="Log severity" className="h-[26px] gap-0.5 rounded-md p-0.5">
                    {Object.entries(levels).map(([value, label]) => (
                      <TabsTrigger
                        key={value}
                        value={value}
                        className="h-[22px] rounded px-2 text-[11px] font-normal data-active:font-bold"
                      >
                        {label}
                      </TabsTrigger>
                    ))}
                  </TabsList>
                </Tabs>
              </TooltipTrigger>
              <TooltipContent className="trace-controls" side="bottom">
                Show this severity and above.
              </TooltipContent>
            </Tooltip>
            <span aria-hidden className="mx-1 h-5 shrink-0 border-l" />
            <FilterSearchInput
              label="Search logs"
              canonical={serializeLogQuery(search)}
              validate={parseLogQuery}
              suggest={suggest}
              onApply={(query) => update(parseLogQuery(query))}
            />
          </fieldset>
          <div className="ml-auto flex shrink-0 items-center justify-end gap-1.5">
            <TimeRangeSelect
              value={search.range ?? "24h"}
              label="Log time range"
              timezone={timezone}
              onChange={(range) => update({ range })}
            />
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="size-[26px]"
                    aria-label="Clear date range"
                    disabled={!search.range && !search.from && !search.to}
                    onClick={() => {
                      form.clearErrors();
                      update({ range: undefined, from: undefined, to: undefined });
                    }}
                  >
                    <XIcon className="size-3.5" />
                  </Button>
                }
              />
              <TooltipContent side="bottom" className="trace-controls">
                Reset to last 24 hours
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="size-[26px]"
                    aria-label={live ? "Pause live logs" : "Live logs"}
                    aria-pressed={live}
                    disabled={search.range === "custom"}
                    onClick={() => setLive((value) => !value)}
                  >
                    {live ? <PauseIcon className="size-3.5" /> : <PlayIcon className="size-3.5" />}
                  </Button>
                }
              />
              <TooltipContent side="bottom" className="trace-controls">
                {live ? "Pause live updates" : "Show latest logs every 3 seconds"}
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="size-[26px]"
                    aria-label="Refresh logs"
                    onClick={() => setAttempt((a) => a + 1)}
                  >
                    <RefreshCwIcon className="size-3.5" />
                  </Button>
                }
              />
              <TooltipContent side="bottom" className="trace-controls">
                Refresh logs
              </TooltipContent>
            </Tooltip>
          </div>
        </div>
        {search.range === "custom" && (
          <form
            className="border-t bg-muted/10 px-3 py-1.5"
            onSubmit={(event) => event.preventDefault()}
          >
            <fieldset className="flex items-center gap-2">
              <Controller
                control={form.control}
                name="from"
                render={({ field }) => (
                  <TraceDateTimePicker
                    value={field.value}
                    label="Start"
                    timezone={timezone}
                    onChange={(value) => {
                      field.onChange(value);
                      applyRange();
                    }}
                  />
                )}
              />
              <ArrowRightIcon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
              <Controller
                control={form.control}
                name="to"
                render={({ field }) => (
                  <TraceDateTimePicker
                    value={field.value}
                    label="End"
                    timezone={timezone}
                    onChange={(value) => {
                      field.onChange(value);
                      applyRange();
                    }}
                  />
                )}
              />
            </fieldset>
            {form.formState.errors.root?.message && (
              <p role="alert" className="pt-1 text-[11px] text-destructive">
                {form.formState.errors.root.message}
              </p>
            )}
          </form>
        )}
      </div>
      {error && (
        <p
          role="alert"
          className="shrink-0 border-b bg-muted/20 px-3 py-2 text-xs text-muted-foreground"
        >
          {error}
        </p>
      )}
      <ActivityChart
        key={`${agentId}:${attempt}`}
        kind="Log"
        fromTime={activityFilter.fromTime}
        toTime={activityFilter.toTime}
        load={loadMetrics}
        selectedRange={selectedRange}
        onRange={(from, to) => {
          setActivityWindow(
            (previous) =>
              previous ?? {
                fromTime: filter.fromTime,
                toTime: filter.toTime,
                previous: { range: search.range, from: search.from, to: search.to },
              },
          );
          update({ range: "custom", from, to }, true);
        }}
        onClear={() => {
          if (activityWindow) update(activityWindow.previous, true);
        }}
      />
      <LogTable
        key={`${agentId}:${key}:${attempt}`}
        records={records}
        busy={busy}
        error={error}
        hasMore={!!cursor && !live}
        onMore={() => void more()}
        onSelect={(record) => {
          setLive(false);
          setSelected(record);
        }}
      />
      <Sheet
        open={!!selected}
        onOpenChange={(open) => {
          if (!open) setSelected(undefined);
        }}
      >
        <SheetContent
          side="right"
          className="w-full! gap-0 overflow-hidden p-0 sm:max-w-none! md:w-[min(960px,85vw)]!"
        >
          <header className="flex h-12 shrink-0 items-center justify-between border-b px-3 pr-12">
            <SheetTitle className="text-sm">Log details</SheetTitle>
            <SheetDescription className="text-[11px]">
              {selected
                ? `${date(selected.timestamp)} · ${selected.severity || "Unspecified severity"}`
                : ""}
            </SheetDescription>
          </header>
          {selected && (
            <>
              <div className="flex shrink-0 flex-wrap gap-x-4 gap-y-1 border-b px-3 py-2 text-[11px]">
                <span>Service: {selected.service || "—"}</span>
                <span>Scope: {selected.scope}</span>
                <span>Deployment: {selected.deploymentId || "—"}</span>
                <span>Invocation: {selected.invocationId || "—"}</span>
                <span>Span: {selected.spanId || "—"}</span>
                <span>Session: {selected.threadId || "—"}</span>
                <span>Run: {selected.runId || "—"}</span>
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
              <Tabs defaultValue="message" className="min-h-0 flex-1 gap-0">
                <TabsList
                  aria-label="Log details"
                  className="h-8 w-full shrink-0 justify-start gap-0 rounded-none border-b bg-muted/20 p-0"
                >
                  {["message", "metadata"].map((value) => (
                    <TabsTrigger
                      key={value}
                      value={value}
                      className="h-8 rounded-none border-b-2 border-transparent px-3 text-xs data-active:border-primary data-active:bg-transparent data-active:shadow-none"
                    >
                      {value === "message" ? "Message" : "Metadata"}
                    </TabsTrigger>
                  ))}
                </TabsList>
                <TabsContent value="message" className="min-h-0 flex-1 overflow-auto p-3">
                  <pre className="tracing-table whitespace-pre-wrap break-words text-[11px]">
                    {selected.body}
                  </pre>
                </TabsContent>
                <TabsContent value="metadata" className="min-h-0 flex-1 overflow-auto p-3">
                  <TracePayload
                    text={JSON.stringify({
                      timestamp: selected.timestamp,
                      severity: selected.severity,
                      service: selected.service,
                      invocationId: selected.invocationId,
                      traceId: selected.traceId,
                      spanId: selected.spanId,
                      threadId: selected.threadId,
                      runId: selected.runId,
                      scope: selected.scope,
                      deploymentId: selected.deploymentId,
                      scopeName: selected.scopeName,
                      attributes: selected.attributes,
                      resource: selected.resourceAttributes,
                      scopeAttributes: selected.scopeAttributes,
                    })}
                  />
                </TabsContent>
              </Tabs>
            </>
          )}
        </SheetContent>
      </Sheet>
    </section>
  );
}
