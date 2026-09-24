import type { TranscriptEntry } from "@trytilde/chat-ui";
import type { Observation } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { date } from "./format";
import { observationKey } from "./records";

type Entry = TranscriptEntry;
type Source = Pick<Entry, "id" | "sourceId" | "timestamp" | "timestampLabel">;
const record = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === "object" && !Array.isArray(v);
const str = (v: unknown) => (typeof v === "string" ? v : "");
function parse(value: unknown): unknown {
  if (typeof value !== "string" || value.length > 200_000) return value;
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}
function display(value: unknown): string | undefined {
  if (value === undefined || value === "") return undefined;
  let decoded = parse(value);
  if (
    record(decoded) &&
    ["text", "json", "error-text", "error-json"].includes(str(decoded.type)) &&
    "value" in decoded
  )
    decoded = decoded.value;
  const text = typeof decoded === "string" ? decoded : JSON.stringify(decoded, null, 2);
  return text.length > 200_000
    ? `${text.slice(0, 200_000)}\n[Truncated; open the trace for the full payload]`
    : text;
}
function entries(
  value: unknown,
  source: Source,
  role: "user" | "assistant" | "system",
  depth = 0,
): Entry[] {
  if (depth > 8 || value == null) return [];
  if (typeof value === "string")
    return value ? [{ ...source, kind: "message", role, text: display(value)! }] : [];
  if (Array.isArray(value)) return value.flatMap((v) => entries(v, source, role, depth + 1));
  if (!record(value))
    return typeof value === "number" || typeof value === "boolean"
      ? [{ ...source, kind: "message", role, text: JSON.stringify(value) }]
      : [];
  const type = str(value.type);
  const callId = str(
    value.toolCallId || value.tool_call_id || value.tool_use_id || value.call_id || value.id,
  );
  const fn = record(value.function) ? value.function : undefined;
  const name = str(value.toolName || value.name || fn?.name) || "Tool";
  if (["tool-call", "tool_use", "function_call"].includes(type) || fn) {
    return [
      {
        ...source,
        kind: "tool",
        callId,
        name,
        input: display(value.args ?? value.arguments ?? value.input ?? fn?.arguments),
      },
    ];
  }
  if (value.role === "tool" && Array.isArray(value.content)) {
    return entries(value.content, source, "assistant", depth + 1);
  }
  if (
    ["tool-result", "tool_result", "function_call_output"].includes(type) ||
    value.role === "tool"
  ) {
    return [
      {
        ...source,
        kind: "tool",
        callId,
        name,
        output: display(
          "output" in value ? value.output : "result" in value ? value.result : value.content,
        ),
        error:
          value.is_error || (record(value.output) && str(value.output.type).startsWith("error-"))
            ? display(value.output) || "Tool failed"
            : undefined,
      },
    ];
  }
  if (["reasoning", "thinking", "reasoning_text"].includes(type)) {
    const text =
      str(value.text || value.thinking) ||
      (Array.isArray(value.summary)
        ? value.summary.map((v) => (record(v) ? str(v.text) : "")).join("\n")
        : "");
    return text ? [{ ...source, kind: "reasoning", text }] : [];
  }
  if (["image", "image_url", "input_image", "file", "input_audio"].includes(type)) {
    return [
      {
        ...source,
        kind: "message",
        role,
        text: `[${type.includes("image") ? "Image" : "Attachment"} recorded in trace]`,
      },
    ];
  }
  const messageRole =
    value.role === "user"
      ? "user"
      : ["system", "developer"].includes(str(value.role))
        ? "system"
        : value.role === "assistant"
          ? "assistant"
          : role;
  const result: Entry[] = [];
  for (const key of ["reasoning", "reasoning_content", "thinking"]) {
    const text = str(value[key]);
    if (text) result.push({ ...source, kind: "reasoning", text });
    else if (Array.isArray(value[key]))
      result.push(...entries(value[key], source, "assistant", depth + 1));
  }
  const content =
    value.messages ??
    value.content ??
    value.message ??
    value.text ??
    value.output_text ??
    value.prompt;
  if (content !== undefined)
    result.push(...entries(parse(content), source, messageRole, depth + 1));
  if (Array.isArray(value.choices))
    result.push(
      ...value.choices.flatMap((v) =>
        record(v) ? entries(v.message ?? v.delta ?? v.text, source, "assistant", depth + 1) : [],
      ),
    );
  if (content === undefined && Array.isArray(value.output))
    result.push(...entries(value.output, source, "assistant", depth + 1));
  const calls = value.toolCalls ?? value.tool_calls;
  if (Array.isArray(calls))
    for (const call of calls)
      if (record(call))
        result.push(...entries({ ...call, type: "tool-call" }, source, "assistant", depth + 1));
  if (!result.length && Object.keys(value).length)
    result.push({
      ...source,
      kind: "message",
      role: messageRole,
      text: `\`\`\`json\n${display(value)}\n\`\`\``,
    });
  if (value.role) {
    const joined: Entry[] = [];
    for (const entry of result) {
      const last = joined.at(-1);
      if (entry.kind === "message" && last?.kind === "message" && entry.role === last.role)
        last.text += `\n\n${entry.text}`;
      else joined.push(entry);
    }
    return joined;
  }
  return result;
}
function signature(entry: Entry) {
  if (entry.kind === "message") return JSON.stringify([entry.kind, entry.role, entry.text]);
  if (entry.kind === "tool")
    return JSON.stringify([entry.kind, entry.callId || entry.name, entry.input, entry.output]);
  return JSON.stringify([entry.kind, entry.text]);
}

/** Traces are observations, not a message store. Reconcile successive model history
 * snapshots by sequence overlap, never a global text set (which would erase repeated
 * user messages). Tool IDs join recorded calls/results and their instrumented spans.
 */
export function sessionTranscript(observations: Observation[]): Entry[] {
  const sorted = [...observations].sort(
    (a, b) => Date.parse(a.startTime) - Date.parse(b.startTime) || a.id.localeCompare(b.id),
  );
  const byId = new Map(sorted.map((row) => [observationKey(row), row]));
  const parentGenerations = new Set<string>();
  const generationTraces = new Set<string>();
  for (const row of sorted.filter((row) => row.type === "GENERATION")) {
    generationTraces.add(row.traceId);
    const seen = new Set<string>();
    let parent = row.parentId;
    while (parent && !seen.has(parent)) {
      seen.add(parent);
      const key = `${row.traceId}:${parent}`;
      const ancestor = byId.get(key);
      if (!ancestor) break;
      if (ancestor.type === "GENERATION") parentGenerations.add(key);
      parent = ancestor.parentId;
    }
  }
  const result: Entry[] = [];
  const inherited = new Map<string, Entry[]>();
  const capturedReasoning = (row: Observation): Entry[] => {
    const source = {
      id: "",
      sourceId: observationKey(row),
      timestamp: row.startTime,
      timestampLabel: row.startTime ? date(row.startTime) : "",
    };
    const parts = entries(parse(row.output), source, "assistant").filter(
      (entry) => entry.kind === "reasoning",
    );
    for (const attribute of row.attributes) {
      if (
        !["ai.response.reasoning", "gen_ai.response.reasoning"].includes(
          attribute.key.replace(/^attributes\./, ""),
        )
      )
        continue;
      const text = parse(attribute.value);
      if (typeof text === "string" && text) parts.push({ ...source, kind: "reasoning", text });
    }
    return parts;
  };
  const leafThoughts = new Set(
    sorted
      .filter((row) => row.type === "GENERATION" && !parentGenerations.has(observationKey(row)))
      .flatMap((row) =>
        capturedReasoning(row).map((entry) => `${row.traceId}:${signature(entry)}`),
      ),
  );
  for (const row of sorted.filter((row) => parentGenerations.has(observationKey(row)))) {
    const thoughts = capturedReasoning(row).filter(
      (entry) => !leafThoughts.has(`${row.traceId}:${signature(entry)}`),
    );
    inherited.set(row.traceId, [...(inherited.get(row.traceId) ?? []), ...thoughts]);
  }
  const history: string[] = [];
  const tools = new Map<string, Extract<Entry, { kind: "tool" }>>();
  const thoughts = new Set<string>();
  let previousTrace = "";
  let serial = 0;
  function append(entry: Entry, row: Observation) {
    if (entry.kind === "tool") {
      const key = `${row.traceId}:${entry.callId || `${entry.name}:${entry.input || ""}`}`;
      const existing = tools.get(key);
      if (existing) {
        existing.input ??= entry.input;
        existing.output ??= entry.output;
        existing.error ??= entry.error;
        if (existing.name === "Tool" && entry.name !== "Tool") existing.name = entry.name;
        // Prefer the real tool span when opening trace details.
        if (row.type === "TOOL") existing.sourceId = entry.sourceId;
        return;
      }
      tools.set(key, entry);
    }
    if (entry.kind === "reasoning") {
      const key = `${entry.sourceId}:${entry.text}`;
      if (thoughts.has(key)) return;
      thoughts.add(key);
    }
    entry.id = `${entry.sourceId}:${serial++}`;
    result.push(entry);
  }
  for (const row of sorted) {
    const source = {
      id: "",
      sourceId: observationKey(row),
      timestamp: row.startTime,
      timestampLabel: row.startTime ? date(row.startTime) : "",
    };
    const attrs = Object.fromEntries(
      row.attributes.map((a) => [a.key.replace(/^attributes\./, ""), parse(a.value)]),
    );
    if (row.type === "TOOL") {
      // SDK/RPC instrumentation can wrap the same tool execution without a payload.
      if (!row.input && !row.output && byId.get(`${row.traceId}:${row.parentId}`)?.type === "TOOL")
        continue;
      append(
        {
          ...source,
          kind: "tool",
          callId: str(attrs["ai.toolCall.id"] || attrs["gen_ai.tool.call.id"]),
          name: str(attrs["ai.toolCall.name"] || attrs["gen_ai.tool.name"]) || row.name,
          input: display(row.input),
          output: display(row.output),
          error: row.level === "ERROR" ? row.statusMessage || "Tool failed" : undefined,
        },
        row,
      );
      continue;
    }
    if (parentGenerations.has(observationKey(row))) continue;
    if (row.type !== "GENERATION" && !row.input && !row.output) continue;
    if (
      row.type !== "GENERATION" &&
      (generationTraces.has(row.traceId) || byId.has(`${row.traceId}:${row.parentId}`))
    )
      continue;
    const incoming = entries(parse(row.input), source, "user");
    const sequence = incoming.filter((e) => e.kind !== "reasoning");
    const keys = sequence.map(signature);
    let overlap =
      previousTrace === row.traceId || keys.length > 1 ? Math.min(history.length, keys.length) : 0;
    while (
      overlap > 0 &&
      !keys.slice(0, overlap).every((key, i) => key === history[history.length - overlap + i])
    )
      overlap--;
    let index = 0;
    for (const entry of incoming) {
      if (entry.kind === "reasoning") append(entry, row);
      else if (index++ >= overlap) {
        history.push(signature(entry));
        append(entry, row);
      }
    }
    for (const entry of inherited.get(row.traceId) ?? []) append(entry, row);
    inherited.delete(row.traceId);
    // Provider reasoning and tool calls can be attributes even when output is plain text.
    for (const key of ["ai.response.reasoning", "gen_ai.response.reasoning"]) {
      if (typeof attrs[key] === "string" && attrs[key])
        append({ ...source, kind: "reasoning", text: attrs[key] }, row);
    }
    const outgoing = entries(parse(row.output), source, "assistant");
    for (const entry of outgoing) {
      if (entry.kind !== "reasoning") history.push(signature(entry));
      append(entry, row);
    }
    const callPayload = attrs["ai.response.toolCalls"];
    if (Array.isArray(callPayload))
      for (const call of callPayload)
        if (record(call)) {
          for (const entry of entries({ ...call, type: "tool-call" }, source, "assistant")) {
            const key = signature(entry);
            if (!history.includes(key)) {
              history.push(key);
              append(entry, row);
            }
          }
        }
    if (!incoming.length && !outgoing.length && !callPayload)
      append(
        {
          ...source,
          kind: "notice",
          text: `${row.name || "Model call"}: message content was not recorded.`,
        },
        row,
      );
    previousTrace = row.traceId;
  }
  return result;
}
