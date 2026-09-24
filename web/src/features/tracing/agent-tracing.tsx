import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { Controller, useForm } from "@trytilde/connection-ui";
import { traces } from "@/client";
import {
  TracingState,
  type GetTracingStatusResponse,
  type Observation,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { Button } from "@/components/ui/button";
import { TraceDateTimePicker } from "./date-time-picker";
import { TimeRangeSelect } from "../observability/time-range-select";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ArrowRightIcon, RefreshCwIcon, XIcon } from "lucide-react";
import { ExternalLink } from "./links";
import { Inspector } from "./inspector";
import { TraceActivityChart } from "./activity-chart";
import { TraceGrid } from "./trace-grid";
import { TraceSearchInput } from "./search-input";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { ranges, parseSearch, type TraceSearch } from "./search";

const presets = { all: "All", llm: "LLM Calls", tool: "Tool Calls", errors: "Errors" };

function localDate(value?: string) {
  if (!value) return "";
  const date = new Date(value);
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 19);
}
export function AgentTracing({
  agentId,
  view = "observations",
}: {
  agentId: string;
  view?: "observations" | "sessions";
}) {
  const search = parseSearch(useSearch({ strict: false }) as Record<string, unknown>);
  const navigate = useNavigate();
  const [status, setStatus] = useState<GetTracingStatusResponse>();
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const [observations, setObservations] = useState<Observation[]>([]);
  const [activityWindow, setActivityWindow] = useState<{
    fromTime: string;
    toTime: string;
    previous: Pick<TraceSearch, "range" | "from" | "to">;
  } | null>(null);
  function update(patch: TraceSearch, preserveActivity = false) {
    if (!preserveActivity && ("range" in patch || "from" in patch || "to" in patch))
      setActivityWindow(null);
    setError("");
    void navigate({ to: ".", search: (previous) => ({ ...previous, ...patch }), replace: true });
  }
  const form = useForm<{ from: string; to: string }>({
    defaultValues: { from: localDate(search.from), to: localDate(search.to) },
  });
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
    form.clearErrors();
    update({ from: new Date(from).toISOString(), to: new Date(to).toISOString() });
  }
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
    const now = new Date();
    form.reset({
      from: localDate(search.from || new Date(now.getTime() - 86400_000).toISOString()),
      to: localDate(search.to || now.toISOString()),
    });
  }, [search.from, search.to, search.range]);
  // Freeze the time window across cursor pages. Only a filter change or explicit
  // refresh establishes a new window, so adjacent requests have identical filters.
  const filter = useMemo(() => {
    const end = new Date();
    const milliseconds = ranges[(search.range ?? "24h") as keyof typeof ranges] ?? ranges["24h"];
    return {
      fromTime:
        search.range === "custom" && search.from
          ? new Date(search.from).toISOString()
          : (activityWindow?.fromTime ?? new Date(end.getTime() - milliseconds).toISOString()),
      toTime:
        search.range === "custom" && search.to
          ? new Date(search.to).toISOString()
          : (activityWindow?.toTime ?? end.toISOString()),
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
  }, [queryKey, attempt, activityWindow]);
  // Brushing filters the table while metrics keep the original time horizon.
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
    (Date.parse(filter.fromTime) !== Date.parse(activityWindow.fromTime) ||
      Date.parse(filter.toTime) !== Date.parse(activityWindow.toTime))
      ? { from: filter.fromTime, to: filter.toTime }
      : null;
  useEffect(() => {
    const abort = new AbortController();
    setError("");
    setStatus(undefined);
    void traces
      .getTracingStatus({ agentId }, { signal: abort.signal })
      .then((status) => {
        if (!abort.signal.aborted) setStatus(status);
      })
      .catch((error: unknown) => {
        if (!abort.signal.aborted)
          setError(error instanceof Error ? error.message : "Unable to check tracing");
      });
    return () => abort.abort();
  }, [agentId, attempt]);
  const ready = status?.state === TracingState.READY;
  const disabled = status?.state === TracingState.DISABLED;
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  return (
    <TooltipProvider delay={350}>
      <section
        className="flex min-h-0 flex-1 flex-col"
        aria-label={view === "sessions" ? "Agent sessions" : "Agent tracing"}
      >
        <div className="trace-controls relative z-20 shrink-0 border-b bg-background">
          <div
            className="flex min-h-10 w-full flex-wrap items-center gap-2 px-3 py-1.5"
            role="group"
            aria-label="Trace filter bar"
          >
            <fieldset
              disabled={!ready}
              className="flex min-w-0 flex-1 flex-wrap items-center gap-2 disabled:opacity-60"
            >
              <GroupHint text="Filter by model calls, tool calls, or errors.">
                <Tabs
                  value={search.preset ?? "all"}
                  onValueChange={(value) =>
                    update({ preset: String(value), type: undefined, level: undefined })
                  }
                  className="shrink-0 gap-0"
                >
                  <TabsList
                    aria-label="Observation filters"
                    className="h-[26px] gap-0.5 rounded-md p-0.5"
                  >
                    {Object.entries(presets).map(([id, label]) => (
                      <TabsTrigger
                        key={id}
                        value={id}
                        disabled={!ready}
                        className="h-[22px] rounded px-2 text-xs font-normal data-active:font-bold"
                      >
                        {label}
                      </TabsTrigger>
                    ))}
                  </TabsList>
                </Tabs>
              </GroupHint>
              <span aria-hidden="true" className="mx-1 h-5 shrink-0 border-l" />
              <TraceSearchInput
                search={search}
                observations={observations}
                disabled={!ready}
                onApply={update}
              />
            </fieldset>
            <div className="ml-auto flex shrink-0 items-center justify-end gap-1.5">
              <TimeRangeSelect
                value={search.range ?? "24h"}
                disabled={!ready}
                label="Date range"
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
                      disabled={!ready || (!search.range && !search.from && !search.to)}
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
                      aria-label="Refresh"
                      className="size-[26px]"
                      onClick={() => setAttempt((n) => n + 1)}
                    >
                      <RefreshCwIcon className="size-3.5" />
                    </Button>
                  }
                />
                <TooltipContent side="bottom" className="trace-controls">
                  Refresh traces
                </TooltipContent>
              </Tooltip>
              <ExternalLink href={status?.projectUrl} label="Open project in Langfuse" compact />
            </div>
          </div>
          {search.range === "custom" && (
            <form
              className="border-t bg-muted/10 px-3 py-1.5"
              onSubmit={(event) => event.preventDefault()}
            >
              <fieldset disabled={!ready} className="flex items-center gap-2">
                <Controller
                  control={form.control}
                  name="from"
                  render={({ field }) => (
                    <TraceDateTimePicker
                      value={field.value}
                      label="Start"
                      timezone={timezone}
                      disabled={!ready}
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
                      disabled={!ready}
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
        {(!ready || error) && (
          <div
            role={error || status?.state === TracingState.UNAVAILABLE ? "alert" : "status"}
            className="shrink-0 border-b bg-muted/20 px-3 py-2 text-xs text-muted-foreground"
          >
            {error || status?.message || "Checking tracing availability…"}
            {disabled && (
              <p className="mt-1">
                Your administrator can enable Langfuse for this installation. Trace uploads are
                discarded while it is disabled.
              </p>
            )}
          </div>
        )}
        {ready && (
          <TraceActivityChart
            key={`activity:${agentId}:${attempt}`}
            agentId={agentId}
            filter={activityFilter}
            refresh={attempt > 0}
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
        )}
        <TraceGrid
          key={`${agentId}:${queryKey}:${attempt}`}
          agentId={agentId}
          filter={filter}
          refresh={attempt > 0}
          view={view}
          ready={ready}
          onInspect={update}
          onObservations={setObservations}
        />
        {ready && (search.trace || search.inspectSession) && (
          <Inspector
            key={search.trace || search.inspectSession}
            agentId={agentId}
            traceId={search.trace}
            sessionId={search.inspectSession}
            selectedId={search.observation}
            initialObservation={observations.find(
              (row) => row.traceId === search.trace && row.id === search.observation,
            )}
            filter={filter}
            onClose={() =>
              update({ trace: undefined, observation: undefined, inspectSession: undefined })
            }
          />
        )}
      </section>
    </TooltipProvider>
  );
}

function GroupHint({ text, children }: { text: string; children: ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<div className="shrink-0" />}>{children}</TooltipTrigger>
      <TooltipContent side="bottom" className="trace-controls">
        {text}
      </TooltipContent>
    </Tooltip>
  );
}
