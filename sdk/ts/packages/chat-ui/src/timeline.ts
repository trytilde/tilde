import type { Activity, Message, ToolCall } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";

/** Where an entry happened: its time, with the activity sequence breaking ties. */
type Place = { at: number; sequence?: bigint };
/** One stretch of an invocation's reasoning, unbroken by a tool call or message. */
type Thought = Place & { key: string; invocation: string; text: string };
/**
 * What the transcript knows about activity, built the same way from the watch stream and from
 * a reload's `recentActivity` so both place entries alike.
 */
export type Activities = {
  /** Each tool call's first sighting. */
  tools: Record<string, Place>;
  /** Each message's first creation event; edits, deletions and aborts never move it. */
  messages: Record<string, Place>;
  thoughts: Thought[];
  /** A tool call or message arrived since the last reasoning, which starts a new stretch. */
  broken: boolean;
};
export const noActivity: Activities = { tools: {}, messages: {}, thoughts: [], broken: false };
const creations = new Set(["message.created", "message.started", "message.completed"]);

export type TimelineItem = { key: string; at: number } & (
  | { kind: "message"; message: Message }
  | { kind: "tools"; tools: ToolCall[] }
  | { kind: "reasoning"; invocation: string; text: string }
);

export function record(state: Activities, activities: Activity[]): Activities {
  let { tools, messages, thoughts, broken } = state;
  for (const activity of activities) {
    const place = {
      at: activity.createdAt ? millis(activity.createdAt) : Date.now(),
      sequence: activity.sequence,
    };
    if (activity.detail.case === "toolCall") {
      const id = activity.detail.value.id;
      if (!tools[id]) tools = { ...tools, [id]: place };
      broken = true;
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
  return { tools, messages, thoughts, broken };
}

/**
 * Messages, tool calls and reasoning in the order they happened, each placed by its first
 * activity (gateway time, then sequence). A message no loaded activity created (older history,
 * or a just-posted message whose activity has not arrived) falls back to its own `createdAt`.
 * A reload replays only recent activity, so a call or message whose first event fell outside it
 * is placed at its first event inside it. Consecutive tool calls share one block; a message or
 * reasoning between them starts a new one.
 */
export function timeline(
  messages: Message[],
  tools: ToolCall[],
  activities: Activities,
): TimelineItem[] {
  type Entry = { item: TimelineItem; place: Place; tool?: ToolCall };
  const entries: Entry[] = [
    ...messages.map((message) => {
      const place = activities.messages[message.id] ?? {
        at: message.createdAt ? millis(message.createdAt) : 0,
      };
      return {
        item: { kind: "message" as const, key: `message:${message.id}`, at: place.at, message },
        place,
      };
    }),
    ...tools.map((tool) => {
      // Tool calls only arrive as activity, so every one has a place; now is a safe default.
      const place = activities.tools[tool.id] ?? { at: Date.now() };
      return {
        item: { kind: "tools" as const, key: `tools:${tool.id}`, at: place.at, tools: [tool] },
        place,
        tool,
      };
    }),
    ...activities.thoughts.map((thought) => ({
      item: {
        kind: "reasoning" as const,
        key: thought.key,
        at: thought.at,
        invocation: thought.invocation,
        text: thought.text,
      },
      place: thought,
    })),
  ];
  // One lexicographic key (time, then sequence, then key) keeps the sort consistent.
  const last = BigInt(Number.MAX_SAFE_INTEGER);
  entries.sort((a, b) => {
    if (a.place.at !== b.place.at) return a.place.at - b.place.at;
    const left = a.place.sequence ?? last;
    const right = b.place.sequence ?? last;
    if (left !== right) return left < right ? -1 : 1;
    return a.item.key.localeCompare(b.item.key);
  });
  const items: TimelineItem[] = [];
  for (const entry of entries) {
    const previous = items.at(-1);
    if (entry.tool && previous?.kind === "tools") previous.tools.push(entry.tool);
    else items.push(entry.item);
  }
  return items;
}
function millis(at: { seconds: bigint; nanos: number }) {
  return Number(at.seconds) * 1000 + at.nanos / 1e6;
}
