"use client";
import { MarkdownText } from "./markdown.js";
import { ConversationMessage } from "./dispatch/conversation-message.js";
import {
  ToolChipsBlock,
  type ToolChipRow,
  type ToolChipIcon,
} from "./dispatch/tool-chips-block.js";
import { TraceBlock } from "./dispatch/trace-block.js";
type Source = { id: string; sourceId: string; timestamp: string; timestampLabel: string };
export type TranscriptEntry = Source &
  (
    | { kind: "message"; role: "user" | "assistant" | "system"; text: string }
    | { kind: "reasoning"; text: string }
    | {
        kind: "tool";
        name: string;
        callId?: string;
        input?: string;
        output?: string;
        error?: string;
      }
    | { kind: "notice"; text: string }
  );
export interface TranscriptProps {
  entries: TranscriptEntry[];
  onInspect?: (sourceId: string) => void;
  className?: string;
}

/** A read-only adapter over Dispatch's original message and execution components. */
export function Transcript({ entries, onInspect, className = "" }: TranscriptProps) {
  const groups: TranscriptEntry[][] = [];
  for (const entry of entries) {
    const previous = groups.at(-1);
    const execution = entry.kind === "tool" || entry.kind === "reasoning";
    if (execution && previous && (previous[0].kind === "tool" || previous[0].kind === "reasoning"))
      previous.push(entry);
    else groups.push([entry]);
  }
  return (
    <div className={`tilde-chat rich-chat ${className}`} aria-label="Recorded conversation">
      <div className="conversation">
        <div className="message-list">
          {groups.map((group) => {
            const first = group[0];
            if (first.kind === "message" && first.role !== "system")
              return (
                <ConversationMessage
                  key={first.id}
                  role={first.role}
                  createdAt={first.timestamp}
                  timestampLabel={first.timestampLabel}
                  onInspect={onInspect ? () => onInspect(first.sourceId) : undefined}
                >
                  <MarkdownText text={first.text} allowImages={false} />
                </ConversationMessage>
              );
            const toolCount = group.filter((entry) => entry.kind === "tool").length;
            return (
              <article
                key={first.id}
                className="message-block"
                aria-label={
                  toolCount
                    ? "Tool calls"
                    : first.kind === "reasoning"
                      ? "Reasoning"
                      : "Recorded context"
                }
              >
                {first.kind === "message" ? (
                  <TraceBlock
                    activeLabel="Instructions"
                    doneLabel="Instructions"
                    rows={[{ id: first.id, primary: first.text, prose: true }]}
                  />
                ) : first.kind === "notice" ? (
                  <p className="trace-notice">{first.text}</p>
                ) : toolCount ? (
                  <ToolChipsBlock
                    headerLabel={`${toolCount} tool call${toolCount === 1 ? "" : "s"}`}
                    rows={group.map(toolRow)}
                  />
                ) : (
                  <TraceBlock
                    activeLabel="Reasoning"
                    doneLabel="Reasoning"
                    rows={group.flatMap((entry) =>
                      entry.kind === "reasoning"
                        ? entry.text
                            .split(/\n{2,}/)
                            .filter(Boolean)
                            .map((text, i) => ({
                              id: `${entry.id}:${i}`,
                              primary: text,
                              prose: true,
                            }))
                        : [],
                    )}
                  />
                )}
                {onInspect && (
                  <div className="trace-sources">
                    {Array.from(
                      new Map(group.map((entry) => [entry.sourceId, entry])).values(),
                    ).map((entry) => (
                      <button
                        key={entry.sourceId}
                        className="trace-source"
                        type="button"
                        onClick={() => onInspect(entry.sourceId)}
                      >
                        {entry.kind === "tool" ? `View ${entry.name} trace` : "View trace"}
                      </button>
                    ))}
                  </div>
                )}
              </article>
            );
          })}
        </div>
      </div>
    </div>
  );
}
function toolRow(entry: TranscriptEntry): ToolChipRow {
  if (entry.kind === "reasoning") {
    const paragraphs = entry.text.split(/\n{2,}/).filter(Boolean);
    return {
      id: entry.id,
      icon: "think",
      label: "Thinking",
      chip: compact(paragraphs[0] ?? ""),
      detail: paragraphs.map((text) => ({ text })),
      detailMono: false,
    };
  }
  if (entry.kind !== "tool") return { id: entry.id, label: "Recorded event" };
  let args: unknown = entry.input;
  try {
    args = JSON.parse(entry.input ?? "");
  } catch {
    /* Already readable text. */
  }
  let chip = typeof args === "string" ? args : "";
  if (args && typeof args === "object") {
    const record = args as Record<string, unknown>;
    for (const key of ["command", "file_path", "path", "filename", "file", "query", "url", "cmd"])
      if (typeof record[key] === "string" && record[key]) {
        chip = record[key];
        break;
      }
  }
  return {
    id: entry.id,
    icon: toolIcon(entry.name),
    label: entry.name.replaceAll(/[_-]+/g, " ").replace(/^./, (char) => char.toUpperCase()),
    chip: compact(chip),
    mono: true,
    detailMono: true,
    failed: !!entry.error,
    pending: false,
    detail: [
      { text: "Input" },
      ...(entry.input ?? "Not recorded").split("\n").map((text) => ({ text })),
      { text: "Output" },
      ...(entry.output ?? entry.error ?? "Not recorded")
        .split("\n")
        .map((text) => ({ text, ...(entry.error ? { tone: "error" as const } : {}) })),
      ...(entry.error && entry.output && entry.error !== entry.output
        ? [{ text: entry.error, tone: "error" as const }]
        : []),
    ],
  };
}
// Dispatch's tool icon and summary conventions.
function toolIcon(name: string): ToolChipIcon {
  const value = name.toLowerCase();
  if (/write|edit|create|patch|update/.test(value)) return "write";
  if (/read|get|fetch|view|list|search|glob|grep/.test(value)) return "read";
  if (/run|bash|shell|exec|command|test|build/.test(value)) return "run";
  if (/think|reason|plan/.test(value)) return "think";
  return "tool";
}
function compact(value: string) {
  const flat = value.replaceAll(/\s+/g, " ").trim();
  return flat.length > 64 ? `${flat.slice(0, 61)}…` : flat;
}
