import type { Observation } from "@/gen/tilde/management/v1/tracing_pb.js";
export const observationKey = (row: Observation) => `${row.traceId}:${row.id}`;
/** OTLP retries may be visible as multiple versions while Langfuse converges. */
export function mergeObservations(previous: Observation[], incoming: Observation[]) {
  const rows = new Map(previous.map((row) => [observationKey(row), row]));
  for (const row of incoming) {
    const key = observationKey(row),
      current = rows.get(key);
    if (!current || row.updatedTime > current.updatedTime) rows.set(key, row);
  }
  return [...rows.values()];
}
