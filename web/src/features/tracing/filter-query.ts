import { create } from "@bufbuild/protobuf";
import {
  FilterConditionSchema,
  type FilterCondition,
  type Observation,
} from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { tokens, quote, unquote, type SearchSuggestion } from "../observability/query";
import type { TraceSearch } from "./search";

/**
 * The observation filter model in text. Every term is `field:value` and all terms must hold.
 * String fields: `name:checkout` (contains for name and payloads, equals otherwise),
 * `type:*gen*` contains, `model:gpt*` starts with, `model:*mini` ends with,
 * `level:ERROR|WARNING` any of, `-level:DEBUG` none of, `-name:retry` does not contain.
 * Number fields: `latency:>2`, `tokens:>=1000`, `cost:<0.01`. A dotted key such as
 * `gen_ai.tool.name:search` filters a span attribute with the same operators.
 */
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
/** Fields that only exist in `where`: operators, and numbers. */
export const extraFields = [
  { key: "status", description: "Status message contains text" },
  { key: "latency", description: "Latency in seconds, e.g. latency:>2" },
  { key: "tokens", description: "Total tokens, e.g. tokens:>=1000" },
  { key: "input_tokens", description: "Input tokens" },
  { key: "output_tokens", description: "Output tokens" },
  { key: "cost", description: "Cost in USD, e.g. cost:>0.01" },
  { key: "ttft", description: "Time to first token in seconds" },
] as const;
const numeric = new Set(["latency", "tokens", "input_tokens", "output_tokens", "cost", "ttft"]);
const serverColumn: Record<string, string> = {
  name: "name",
  type: "type",
  level: "level",
  model: "model",
  session: "session_id",
  invocation: "invocation_id",
  input: "input",
  output: "output",
  status: "status_message",
  latency: "latency",
  tokens: "total_tokens",
  input_tokens: "input_tokens",
  output_tokens: "output_tokens",
  cost: "total_cost",
  ttft: "time_to_first_token",
};
/** Fields whose plain `field:value` means contains rather than equals. */
const textual = new Set(["name", "input", "output", "status"]);
const attributeKey = /^[A-Za-z_][\w-]*(\.[\w-]+)+$/;
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
export const clearedSearch = {
  ...Object.fromEntries(searchFields.map(({ key }) => [key, undefined])),
  where: undefined,
} as Pick<TraceSearch, Field | "where">;

export function serializeQuery(search: TraceSearch) {
  return [
    ...searchFields.flatMap(({ key }) => (search[key] ? [`${key}:${quote(search[key])}`] : [])),
    ...(search.where ? [search.where] : []),
  ].join(" ");
}
/** One term of the query as a server condition. */
export function condition(term: string): FilterCondition {
  const negated = term.startsWith("-");
  const text = negated ? term.slice(1) : term;
  const colon = text.startsWith('"') ? -1 : text.indexOf(":");
  if (colon === -1) throw new Error("Unknown filter. Choose a suggested field.");
  const name = text.slice(0, colon);
  const key = name.toLowerCase();
  const raw = text.slice(colon + 1);
  const attribute = !fields.has(key) && !numeric.has(key) && key !== "status";
  if (attribute && !attributeKey.test(name))
    throw new Error(
      `Unknown filter “${name}”. Choose a suggested field, or a dotted span attribute such as gen_ai.tool.name.`,
    );
  let value = unquote(raw);
  if (!value.trim()) throw new Error(`Enter a value after ${name}:.`);
  if (value.length > 4096) throw new Error(`${name}: is too long.`);
  const column = attribute ? "metadata" : serverColumn[key];
  const build = (operator: string, values: string[]) =>
    create(FilterConditionSchema, { column, operator, values, key: attribute ? name : "" });
  const comparison = !raw.startsWith('"') && /^(>=|<=|>|<)/.exec(value);
  if (comparison) {
    const number = value.slice(comparison[0].length).trim();
    if (!/^-?\d+(\.\d+)?$/.test(number))
      throw new Error(`${name}: ${comparison[0]} needs a number.`);
    if (!attribute && !numeric.has(key))
      throw new Error(`${name}: accepts a value, without comparison operators or groups.`);
    if (negated) throw new Error("Combine - with a value, not a comparison.");
    return build(comparison[0], [number]);
  }
  if (!raw.startsWith('"') && /^[=()]/.test(value))
    throw new Error(`${name}: accepts a value, without comparison operators or groups.`);
  if (numeric.has(key)) {
    if (!/^-?\d+(\.\d+)?$/.test(value)) throw new Error(`${name}: needs a number.`);
    if (negated) throw new Error("Combine - with a text value, not a number.");
    return build("=", [value]);
  }
  if (key === "type" || key === "level") value = value.toUpperCase();
  if (key === "invocation" && !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value))
    throw new Error("invocation: requires a UUID.");
  if (!raw.startsWith('"') && value.includes("|")) {
    const values = value.split("|").filter(Boolean);
    if (textual.has(key)) throw new Error(`${name}: matches text; use one value.`);
    return build(negated ? "none of" : "any of", values);
  }
  if (!raw.startsWith('"') && value.includes("*")) {
    const starts = value.endsWith("*");
    const ends = value.startsWith("*");
    const inner = value.replace(/^\*+|\*+$/g, "");
    if (!inner || inner.includes("*"))
      throw new Error("Use *text*, text* or *text; quote a literal asterisk.");
    if (negated && !(starts && ends))
      throw new Error("Combine - with *text* to exclude, not a prefix or suffix.");
    if (starts && ends) return build(negated ? "does not contain" : "contains", [inner]);
    return build(starts ? "starts with" : "ends with", [inner]);
  }
  if (negated) return build(textual.has(key) ? "does not contain" : "none of", [value]);
  return build(textual.has(key) ? "contains" : "=", [value]);
}
/** The server conditions for a search, with presets folded in. */
export function conditions(search: TraceSearch): FilterCondition[] {
  const result = [
    ...searchFields.flatMap(({ key }) => (search[key] ? [`${key}:${quote(search[key])}`] : [])),
    ...tokens(search.where ?? "").items.map((token) => token.text),
  ].map(condition);
  const preset = (column: string, value: string) =>
    result.push(create(FilterConditionSchema, { column, operator: "=", values: [value] }));
  if (search.preset === "llm") preset("type", "GENERATION");
  if (search.preset === "tool") preset("type", "TOOL");
  if (search.preset === "errors") preset("level", "ERROR");
  return result;
}
/**
 * Plain `field:value` terms on the eight basic fields keep their own URL parameters; every
 * other term (operators, repeats, numbers, attributes) goes to `where` verbatim.
 */
export function parseQuery(query: string): TraceSearch {
  if (query.length > 8192) throw new Error("Search is too long.");
  const scanned = tokens(query);
  if (scanned.unfinished) throw new Error("Close the quoted value before searching.");
  const result: TraceSearch = { ...clearedSearch };
  const bare: string[] = [];
  const where: string[] = [];
  for (const { text } of scanned.items) {
    if (["AND", "OR"].includes(text.toUpperCase()) || /^[()]/.test(text))
      throw new Error("Combine field:value terms with spaces. Groups are not supported.");
    const colon = text.startsWith('"') ? -1 : text.indexOf(":");
    if (colon === -1) {
      bare.push(unquote(text));
      continue;
    }
    // Validate every term the same way the server will.
    const parsed = condition(text);
    const key = text.slice(0, colon).toLowerCase();
    const simple =
      fields.has(key) &&
      !text.startsWith("-") &&
      parsed.values.length === 1 &&
      parsed.operator === (textual.has(key) ? "contains" : "=") &&
      result[key as Field] === undefined;
    if (simple) result[key as Field] = parsed.values[0];
    else where.push(text);
  }
  if (bare.length) {
    if (result.name !== undefined) throw new Error("Use plain name text or name:, not both.");
    result.name = bare.join(" ");
    if (result.name.length > 4096) throw new Error("Name search is too long.");
  }
  if (where.length) result.where = where.join(" ");
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
  const prefix = query.slice(token.start, caret).replace(/^-/, "");
  const colon = prefix.indexOf(":");
  const replace = (text: string) => query.slice(0, token.start) + text + query.slice(token.end);
  const loaded = (name: string) =>
    !name.startsWith("resource.") &&
    !name.startsWith("tilde.session.") &&
    !name.startsWith("tilde.observation.");
  if (colon === -1) {
    const known = [...searchFields, ...extraFields]
      .filter((field) => field.key.startsWith(prefix.toLowerCase()))
      .map((field) => ({
        value: replace(`${field.key}:`),
        label: `${field.key}:`,
        description: field.description,
      }));
    const names = new Set<string>();
    if (prefix)
      for (const row of observations)
        for (const { key } of row.attributes)
          if (loaded(key) && attributeKey.test(key) && key.startsWith(prefix)) names.add(key);
    return [
      ...known,
      ...[...names].slice(0, Math.max(0, 12 - known.length)).map((name) => ({
        value: replace(`${name}:`),
        label: `${name}:`,
        description: "Span attribute equals",
      })),
    ];
  }
  const key = prefix.slice(0, colon).toLowerCase();
  const typed = prefix
    .slice(colon + 1)
    .replace(/^"/, "")
    .toLowerCase();
  const field = searchFields.find((field) => field.key === key);
  const name = prefix.slice(0, colon);
  const values = new Set<string>(key === "type" ? types : key === "level" ? levels : []);
  if (field?.column)
    for (const row of observations) {
      const value = row[field.column];
      if (value && value.length <= 160) values.add(value);
    }
  if (!field && attributeKey.test(name))
    for (const row of observations)
      for (const attribute of row.attributes)
        if (attribute.key === name && attribute.value && attribute.value.length <= 160)
          values.add(attribute.value);
  if (!field && !attributeKey.test(name)) return [];
  return [...values]
    .filter((value) => value.toLowerCase().includes(typed))
    .slice(0, 12)
    .map((value) => ({
      value: replace(`${name}:${quote(value)} `),
      label: value,
      description: `${name}: ${key === "type" || key === "level" ? "value" : "loaded value"}`,
    }));
}
