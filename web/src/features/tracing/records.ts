import { create } from "@bufbuild/protobuf";
import { TraceSessionSummarySchema } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import type {
  Observation,
  TraceSessionSummary,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
export const observationKey = (row: Observation) => `${row.traceId}:${row.id}`;
/** A retried span may be loaded twice across pages until the span store collapses it. */
export function mergeObservations(previous: Observation[], incoming: Observation[]) {
  const rows = new Map(previous.map((row) => [observationKey(row), row]));
  for (const row of incoming) {
    const key = observationKey(row),
      current = rows.get(key);
    if (!current || row.updatedTime > current.updatedTime) rows.set(key, row);
  }
  return [...rows.values()];
}

/** Session facts are the latest captured trace metadata; trace metrics are recomputed from the
 * deduplicated loaded observations, never summed across cursor-page summaries. */
export function summarizeLoadedSessions(
  rows: Observation[],
  details: ReadonlyMap<string, TraceSessionSummary> = new Map(),
) {
  const groups = new Map<string, TraceSessionSummary>();
  for (const row of rows) {
    if (!row.sessionId) continue;
    let summary = groups.get(row.sessionId);
    if (!summary) {
      summary = create(TraceSessionSummarySchema, details.get(row.sessionId));
      summary.id = row.sessionId;
      summary.errorCount = 0;
      summary.totalTokens = undefined;
      summary.costUsd = undefined;
      groups.set(row.sessionId, summary);
    }
    if (row.level === "ERROR") summary.errorCount++;
    if (row.totalTokens !== undefined)
      summary.totalTokens = (summary.totalTokens ?? 0) + row.totalTokens;
    if (row.costUsd !== undefined) summary.costUsd = (summary.costUsd ?? 0) + row.costUsd;
  }
  return [...groups.values()].sort(
    (a, b) => (Date.parse(b.lastTurnTime) || 0) - (Date.parse(a.lastTurnTime) || 0),
  );
}
