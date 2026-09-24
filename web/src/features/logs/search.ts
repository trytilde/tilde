import { ranges } from "../tracing/search";
export type LogSearch = {
  range?: string;
  from?: string;
  to?: string;
  severity?: string;
  text?: string;
  invocation?: string;
  trace?: string;
  service?: string;
  session?: string;
  run?: string;
  scope?: string;
};
export function parseLogSearch(raw: Record<string, unknown>): LogSearch {
  const result = Object.fromEntries(
    [
      "range",
      "from",
      "to",
      "severity",
      "text",
      "invocation",
      "trace",
      "service",
      "session",
      "run",
      "scope",
    ].flatMap((key) => (typeof raw[key] === "string" && raw[key] ? [[key, raw[key]]] : [])),
  );
  if (result.range && result.range !== "custom" && !(result.range in ranges)) delete result.range;
  for (const key of ["from", "to"])
    if (result[key] && !Number.isFinite(Date.parse(result[key]))) delete result[key];
  if (result.severity && (!/^\d+$/.test(result.severity) || Number(result.severity) > 24))
    delete result.severity;
  if (result.scope && !["deployment", "invocation"].includes(result.scope)) delete result.scope;
  return result;
}
