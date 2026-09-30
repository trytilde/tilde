import { useCallback } from "react";
import { traces } from "@/client";
import type { ObservationFilter } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { ActivityChart } from "../observability/activity-chart";
export function TraceActivityChart({
  agentId,
  filter,
  refresh,
  onRange,
  onClear,
  selectedRange,
}: {
  agentId: string;
  filter: Partial<Omit<ObservationFilter, "$typeName">>;
  refresh: boolean;
  onRange: (from: string, to: string) => void;
  onClear: () => void;
  selectedRange: { from: string; to: string } | null;
}) {
  const load = useCallback(
    async (signal: AbortSignal, _retry: boolean) => {
      const response = await traces.getObservationMetrics({ agentId, filter }, { signal });
      return response.buckets.map((bucket) => ({ ...bucket, count: bucket.observationCount }));
    },
    [agentId, filter, refresh],
  );
  return (
    <ActivityChart
      kind="Trace"
      showLatency
      fromTime={filter.fromTime!}
      toTime={filter.toTime!}
      load={load}
      onRange={onRange}
      onClear={onClear}
      selectedRange={selectedRange}
    />
  );
}
