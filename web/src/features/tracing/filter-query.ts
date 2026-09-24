import { tokens, quote, unquote, type SearchSuggestion } from "../observability/query";
import type { Observation } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import type { TraceSearch } from "./search";

/** Only advertise filters that the management API can execute on the server. */
export const searchFields = [
  { key: "name", description: "Observation name contains text", column: "name" },
  { key: "type", description: "Observation type", column: "type" },
  { key: "level", description: "Status level", column: "level" },
  { key: "model", description: "Model name", column: "model" },
  { key: "session", description: "Conversation/session ID", column: "sessionId" },
  { key: "invocation", description: "Agent invocation ID", column: "invocationId" },
  { key: "input", description: "Search input text", column: undefined },
  { key: "output", description: "Search output text", column: undefined },
] as const;
type Field = (typeof searchFields)[number]["key"];
const fields = new Set<string>(searchFields.map((field) => field.key));
const types = [
  "GENERATION",
  "TOOL",
  "SPAN",
  "AGENT",
  "CHAIN",
  "EVENT",
  "EMBEDDING",
  "RETRIEVER",
  "EVALUATOR",
  "GUARDRAIL",
];
const levels = ["ERROR", "WARNING", "DEFAULT", "DEBUG"];
export const clearedSearch = Object.fromEntries(
  searchFields.map(({ key }) => [key, undefined]),
) as Pick<TraceSearch, Field>;

export function serializeQuery(search: TraceSearch) {
  return searchFields
    .flatMap(({ key }) => (search[key] ? [`${key}:${quote(search[key])}`] : []))
    .join(" ");
}
export function parseQuery(query: string): TraceSearch {
  if (query.length > 8192) throw new Error("Search is too long.");
  const scanned = tokens(query);
  if (scanned.unfinished) throw new Error("Close the quoted value before searching.");
  const result: TraceSearch = { ...clearedSearch };
  const bare: string[] = [];
  for (const { text } of scanned.items) {
    if (["AND", "OR"].includes(text) || text.startsWith("-"))
      throw new Error(
        "Combine field:value terms with spaces. Groups and exclusions are not supported.",
      );
    const colon = text.startsWith('"') ? -1 : text.indexOf(":");
    if (colon === -1) {
      bare.push(unquote(text));
      continue;
    }
    const key = text.slice(0, colon).toLowerCase();
    if (!fields.has(key)) throw new Error(`Unknown filter “${key}”. Choose a suggested field.`);
    const field = key as Field;
    if (result[field] !== undefined) throw new Error(`Use ${field}: only once.`);
    const raw = text.slice(colon + 1);
    let value = unquote(raw);
    if (!value.trim()) throw new Error(`Enter a value after ${field}:.`);
    if (value.length > 4096) throw new Error(`${field}: is too long.`);
    if (!raw.startsWith('"') && /^[<>=()]/.test(value))
      throw new Error(`${field}: accepts a value, without comparison operators or groups.`);
    if (!raw.startsWith('"') && value.includes("*")) {
      if (field === "name" && /^\*[^*]+\*$/.test(value)) value = value.slice(1, -1);
      else throw new Error("Use name:*text* for contains, or quote a literal asterisk.");
    }
    if (field === "type" || field === "level") value = value.toUpperCase();
    if (field === "invocation" && !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value))
      throw new Error("invocation: requires a UUID.");
    result[field] = value;
  }
  if (bare.length) {
    if (result.name !== undefined) throw new Error("Use plain name text or name:, not both.");
    result.name = bare.join(" ");
    if (result.name.length > 4096) throw new Error("Name search is too long.");
  }
  return result;
}

export function suggestions(
  query: string,
  caret: number,
  observations: Observation[],
): SearchSuggestion[] {
  const scanned = tokens(query).items;
  const token = scanned.find((token) => caret >= token.start && caret <= token.end) ?? {
    start: caret,
    end: caret,
    text: "",
  };
  const prefix = query.slice(token.start, caret);
  const colon = prefix.indexOf(":");
  const replace = (text: string) => query.slice(0, token.start) + text + query.slice(token.end);
  if (colon === -1)
    return searchFields
      .filter((field) => field.key.startsWith(prefix.toLowerCase()))
      .map((field) => ({
        value: replace(`${field.key}:`),
        label: `${field.key}:`,
        description: field.description,
      }));
  const key = prefix.slice(0, colon).toLowerCase();
  const field = searchFields.find((field) => field.key === key);
  if (!field) return [];
  const typed = prefix
    .slice(colon + 1)
    .replace(/^"/, "")
    .toLowerCase();
  const values = new Set<string>(key === "type" ? types : key === "level" ? levels : []);
  if (field.column)
    for (const row of observations) {
      const value = row[field.column];
      if (value && value.length <= 160) values.add(value);
    }
  return [...values]
    .filter((value) => value.toLowerCase().includes(typed))
    .slice(0, 12)
    .map((value) => ({
      value: replace(`${key}:${quote(value)} `),
      label: value,
      description: `${key}: ${key === "type" || key === "level" ? "value" : "loaded value"}`,
    }));
}
