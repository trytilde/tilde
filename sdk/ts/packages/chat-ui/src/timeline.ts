import type { Activity, Message, ToolCall } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";

/** An entry's first activity: its commit-ordered sequence, and its time for messages without one. */
type Place = { sequence: bigint; at: number };
/** One stretch of an invocation's reasoning, unbroken by a tool call or message. */
type Thought = Place & { key: string; invocation: string; text: string };
/**
 * What the transcript knows about activity, built the same way from the watch stream and from
 * a reload's `recentActivity` so both place entries alike.
 */
export type Activities = {
  /** Each tool call's first event. */
  tools: Record<string, Place>;
  /** Each message's creation event; edits, deletions and aborts never move it. */
  messages: Record<string, Place>;
  thoughts: Thought[];
  /** A tool call or message started since the last reasoning, which starts a new stretch. */
  broken: boolean;
};
export const noActivity: Activities = { tools: {}, messages: {}, thoughts: [], broken: false };
// Self-hosted runtimes record a posted message as `message.created`.
const creations = new Set(["message.created", "message.started", "message.completed"]);

export type TimelineItem = { key: string; at: number } & (
  | { kind: "message"; message: Message }
  | { kind: "tools"; tools: ToolCall[] }
  | { kind: "reasoning"; text: string }
);

export function record(state: Activities, activities: Activity[]): Activities {
  let { tools, messages, thoughts, broken } = state;
  for (const activity of activities) {
    const place = {
      sequence: activity.sequence,
      at: activity.createdAt ? millis(activity.createdAt) : 0,
    };
    if (activity.detail.case === "toolCall") {
      const id = activity.detail.value.id;
      if (!tools[id]) {
        tools = { ...tools, [id]: place };
        broken = true;
      }
    } else if (activity.detail.case === "message") {
      const id = activity.detail.value.id;
      if (!messages[id] && creations.has(activity.kind)) {
        messages = { ...messages, [id]: place };
        broken = true;
      }
    } else if (activity.kind === "reasoning.delta" && activity.textDelta) {
      const last = thoughts.at(-1);
      thoughts =
        last && !broken && last.invocation === activity.entityId
          ? [...thoughts.slice(0, -1), { ...last, text: last.text + activity.textDelta }]
          : [
              ...thoughts,
              {
                ...place,
                key: `reasoning:${activity.sequence}`,
                invocation: activity.entityId,
                text: activity.textDelta,
              },
            ];
      broken = false;
    }
  }
  if (
    tools === state.tools &&
    messages === state.messages &&
    thoughts === state.thoughts &&
    broken === state.broken
  )
    return state;
  return { tools, messages, thoughts, broken };
}

/**
 * Messages, tool calls and reasoning in the order they happened. Entries with a first activity
 * follow its sequence, the thread's commit order (activity times mix database and runtime clocks).
 * History and the replayed activity are cut at the same sequence, so a loaded message no activity
 * created came before all of it and goes first, by `createdAt`. A message `posted` here whose
 * activity has not arrived goes last. A reload replays only recent activity, so a call whose first
 * event fell outside it is placed at its first event inside it. Consecutive tool calls share one
 * block; a message or reasoning between them starts a new one.
 */
export function timeline(
  messages: Message[],
  tools: ToolCall[],
  activities: Activities,
  posted: ReadonlySet<string>,
): TimelineItem[] {
  type Entry = { item: TimelineItem; sequence: bigint; tool?: ToolCall };
  const placed: Entry[] = [];
  const earlier: TimelineItem[] = [];
  const pending: TimelineItem[] = [];
  for (const message of messages) {
    const place = activities.messages[message.id];
    const at = place?.at ?? (message.createdAt ? millis(message.createdAt) : 0);
    const item = { kind: "message" as const, key: `message:${message.id}`, at, message };
    if (place) placed.push({ item, sequence: place.sequence });
    else (posted.has(message.id) ? pending : earlier).push(item);
  }
  for (const tool of tools) {
    // Tool calls arrive only as activity, so each has a place.
    const place = activities.tools[tool.id];
    if (place)
      placed.push({
        item: { kind: "tools", key: `tools:${tool.id}`, at: place.at, tools: [tool] },
        sequence: place.sequence,
        tool,
      });
  }
  for (const thought of activities.thoughts)
    placed.push({
      item: { kind: "reasoning", key: thought.key, at: thought.at, text: thought.text },
      sequence: thought.sequence,
    });
  placed.sort((a, b) => (a.sequence < b.sequence ? -1 : a.sequence > b.sequence ? 1 : 0));
  const byTime = (a: TimelineItem, b: TimelineItem) => a.at - b.at || a.key.localeCompare(b.key);
  const items: TimelineItem[] = [];
  for (const { item, tool } of [
    ...earlier.sort(byTime).map((item) => ({ item, tool: undefined })),
    ...placed,
    ...pending.sort(byTime).map((item) => ({ item, tool: undefined })),
  ]) {
    const previous = items.at(-1);
    if (tool && previous?.kind === "tools") previous.tools.push(tool);
    else items.push(item);
  }
  return items;
}
export function millis(at: { seconds: bigint; nanos: number }) {
  return Number(at.seconds) * 1000 + at.nanos / 1e6;
}
