export const ranges = {
  "15m": 15 * 60_000,
  "1h": 3600_000,
  "6h": 6 * 3600_000,
  "24h": 24 * 3600_000,
  "7d": 7 * 86400_000,
  "30d": 30 * 86400_000,
};
export type TraceSearch = {
  view?: string;
  preset?: string;
  range?: string;
  from?: string;
  to?: string;
  name?: string;
  type?: string;
  level?: string;
  model?: string;
  session?: string;
  invocation?: string;
  input?: string;
  output?: string;
  trace?: string;
  observation?: string;
  inspectSession?: string;
};
export function parseSearch(raw: Record<string, unknown>): TraceSearch {
  const values = Object.fromEntries(
    [
      "view",
      "preset",
      "range",
      "from",
      "to",
      "name",
      "type",
      "level",
      "model",
      "session",
      "invocation",
      "input",
      "output",
      "trace",
      "observation",
      "inspectSession",
    ].flatMap((key) => (typeof raw[key] === "string" && raw[key] ? [[key, raw[key]]] : [])),
  );
  if (values.range && values.range !== "custom" && !(values.range in ranges)) delete values.range;
  for (const key of ["from", "to"])
    if (values[key] && !Number.isFinite(Date.parse(values[key]))) delete values[key];
  return values;
}
