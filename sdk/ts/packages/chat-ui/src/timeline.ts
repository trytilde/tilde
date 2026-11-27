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
  thoughts: Thought[];
  /** A tool call or message arrived since the last reasoning, which starts a new stretch. */
  broken: boolean;
};
export const noActivity: Activities = { tools: {}, thoughts: [], broken: false };

export type TimelineItem = { key: string; at: number } & (
  | { kind: "message"; message: Message }
  | { kind: "tools"; tools: ToolCall[] }
  | { kind: "reasoning"; invocation: string; text: string }
);

export function record(state: Activities, activities: Activity[]): Activities {
  let { tools, thoughts, broken } = state;
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
      broken = true;
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
  return { tools, thoughts, broken };
}

/**
 * Messages, tool calls and reasoning in the order they happened. A message is placed by its own
 * creation time (the database clock), which its edits and deletions never change; a tool call and
 * reasoning by their first activity (the gateway clock). A message carries no sequence, and
 * `message.completed` both creates posted messages and ends streamed ones, so time is the shared
 * key. Consecutive tool calls share one block; a message or reasoning between them starts a new one.
 */
export function timeline(
  messages: Message[],
  tools: ToolCall[],
  activities: Activities,
): TimelineItem[] {
  type Entry = { item: TimelineItem; place: Place; tool?: ToolCall };
  const entries: Entry[] = [
    ...messages.map((message) => {
      const at = message.createdAt ? millis(message.createdAt) : 0;
      return {
        item: { kind: "message" as const, key: `message:${message.id}`, at, message },
        place: { at },
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
