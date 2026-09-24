import type { LogRecord } from "@trytilde/contracts/tilde/management/v1/logs_pb.js";
import { tokens, quote, unquote, type SearchSuggestion } from "../observability/query";
import type { LogSearch } from "./search";
const fields = [
  {
    key: "scope",
    property: "scope",
    column: "scope",
    description: "Deployment or invocation logs",
  },
  { key: "message", property: "text", column: "body", description: "Message contains text" },
  { key: "service", property: "service", column: "service", description: "Service name" },
  {
    key: "invocation",
    property: "invocation",
    column: "invocationId",
    description: "Agent invocation ID",
  },
  { key: "trace", property: "trace", column: "traceId", description: "Correlated trace ID" },
  {
    key: "session",
    property: "session",
    column: "threadId",
    description: "Conversation/session ID",
  },
  { key: "run", property: "run", column: "runId", description: "Agent run ID" },
  { key: "severity", property: "severity", column: "severity", description: "Minimum severity" },
] as const;
const severityValues: Record<string, string> = {
  ALL: "0",
  TRACE: "1",
  DEBUG: "5",
  INFO: "9",
  WARN: "13",
  WARNING: "13",
  ERROR: "17",
  FATAL: "21",
};
const severityName = (value: string) =>
  Object.entries(severityValues).find(([, number]) => number === value)?.[0] ?? value;
export function serializeLogQuery(search: LogSearch) {
  return fields
    .flatMap(({ key, property }) =>
      search[property] && search[property] !== "0"
        ? [
            `${key}:${quote(property === "severity" ? severityName(search[property]) : search[property])}`,
          ]
        : [],
    )
    .join(" ");
}
export function parseLogQuery(query: string): LogSearch {
  if (query.length > 8192) throw new Error("Search is too long.");
  const scanned = tokens(query);
  if (scanned.unfinished) throw new Error("Close the quoted value before searching.");
  const result: LogSearch = {
    text: undefined,
    service: undefined,
    invocation: undefined,
    trace: undefined,
    severity: undefined,
    session: undefined,
    run: undefined,
    scope: undefined,
  };
  const bare: string[] = [];
  for (const { text } of scanned.items) {
    if (["AND", "OR"].includes(text) || text.startsWith("-"))
      throw new Error("Combine field:value terms with spaces. Quote literal punctuation.");
    const colon = text.startsWith('"') ? -1 : text.indexOf(":");
    if (colon < 0) {
      bare.push(unquote(text));
      continue;
    }
    const key = text.slice(0, colon).toLowerCase(),
      field = fields.find((f) => f.key === key);
    if (!field) throw new Error(`Unknown filter “${key}”. Choose a suggested field.`);
    if (result[field.property] !== undefined) throw new Error(`Use ${key}: only once.`);
    const raw = text.slice(colon + 1);
    let value = unquote(raw);
    if (!value.trim()) throw new Error(`Enter a value after ${key}:.`);
    if (value.length > 1024) throw new Error(`${key}: is too long.`);
    if (!raw.startsWith('"') && /^[<>=()]/.test(value))
      throw new Error(`${key}: accepts a value without comparison operators.`);
    if (key === "scope" && !["deployment", "invocation"].includes(value))
      throw new Error("Choose deployment or invocation for scope:.");
    if (key === "severity") {
      value = severityValues[value.toUpperCase()] ?? value;
      if (!/^\d+$/.test(value) || Number(value) > 24)
        throw new Error("Choose a severity from TRACE through FATAL.");
    }
    if (
      ["invocation", "session", "run"].includes(key) &&
      !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value)
    )
      throw new Error(`${key}: requires a UUID.`);
    if (key === "trace" && !/^[0-9a-f]{32}$/i.test(value))
      throw new Error("trace: requires a 32-character trace ID.");
    result[field.property] = value;
  }
  if (bare.length) {
    if (result.text !== undefined) throw new Error("Use plain message text or message:, not both.");
    result.text = bare.join(" ");
    if (result.text.length > 1024) throw new Error("Message search is too long.");
  }
  return result;
}
export function logSuggestions(
  query: string,
  caret: number,
  records: LogRecord[],
): SearchSuggestion[] {
  const token = tokens(query).items.find((t) => caret >= t.start && caret <= t.end) ?? {
    start: caret,
    end: caret,
    text: "",
  };
  const prefix = query.slice(token.start, caret),
    colon = prefix.indexOf(":");
  const replace = (text: string) => query.slice(0, token.start) + text + query.slice(token.end);
  if (colon < 0)
    return fields
      .filter((f) => f.key.startsWith(prefix.toLowerCase()))
      .map((f) => ({
        value: replace(`${f.key}:`),
        label: `${f.key}:`,
        description: f.description,
      }));
  const field = fields.find((f) => f.key === prefix.slice(0, colon).toLowerCase());
  if (!field) return [];
  const typed = prefix
    .slice(colon + 1)
    .replace(/^"/, "")
    .toLowerCase();
  const values =
    field.key === "scope"
      ? ["deployment", "invocation"]
      : field.key === "severity"
        ? ["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL"]
        : [
            ...new Set(
              records.map((r) => r[field.column]).filter((value) => value && value.length <= 160),
            ),
          ];
  return values
    .filter((value) => value.toLowerCase().includes(typed))
    .slice(0, 12)
    .map((value) => ({
      value: replace(`${field.key}:${quote(value)} `),
      label: value,
      description: field.description,
    }));
}
