import type { Activity, Message, ToolCall } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";

/** Where an entity first appeared in the thread's activity. The sequence is authoritative;
 * the time orders it against messages no loaded activity covers (older history, or a just
 * posted message whose activity has not arrived). */
export type Seen = { sequence: bigint; at: number };
/** First sighting per entity, keyed `message:<id>`, `tool:<id>` or `reasoning:<invocation id>`. */
export type Order = Record<string, Seen>;

export type TimelineItem =
  | { kind: "message"; key: string; message: Message }
  | { kind: "tools"; key: string; tools: ToolCall[] }
  | { kind: "reasoning"; key: string; invocation: string; text: string };

/** The entity an activity orders, if it is one the transcript shows. */
export function orderKey(activity: Activity): string | undefined {
  if (activity.detail.case === "message") return `message:${activity.detail.value.id}`;
  if (activity.detail.case === "toolCall") return `tool:${activity.detail.value.id}`;
  if (activity.kind === "reasoning.delta" && activity.entityId)
    return `reasoning:${activity.entityId}`;
  return undefined;
}

/** Record activities' first sightings; later events of the same entity keep its place. */
export function see(order: Order, activities: Activity[]): Order {
  const next = { ...order };
  let added = false;
  for (const activity of activities) {
    const key = orderKey(activity);
    if (!key || next[key]) continue;
    next[key] = {
      sequence: activity.sequence,
      at: activity.createdAt ? millis(activity.createdAt) : Date.now(),
    };
    added = true;
  }
  return added ? next : order;
}
function millis(at: { seconds: bigint; nanos: number }) {
  return Number(at.seconds) * 1000 + at.nanos / 1e6;
}

/**
 * Messages, tool calls and reasoning in the order they happened. Consecutive tool calls share
 * one block; a message (or reasoning) between them starts a new one.
 */
export function timeline(
  messages: Message[],
  tools: ToolCall[],
  reasoning: Record<string, string>,
  order: Order,
): TimelineItem[] {
  type Entry = { item: TimelineItem; tool?: ToolCall; seen?: Seen; at: number };
  const entries: Entry[] = [
    ...messages.map((message) => {
      const seen = order[`message:${message.id}`];
      const created = message.createdAt ? millis(message.createdAt) : 0;
      return {
        item: { kind: "message" as const, key: message.id, message },
        seen,
        at: seen?.at ?? created,
      };
    }),
    ...tools.map((tool) => {
      const seen = order[`tool:${tool.id}`];
      return {
        item: { kind: "tools" as const, key: tool.id, tools: [tool] },
        tool,
        seen,
        at: seen?.at ?? Number.MAX_SAFE_INTEGER,
      };
    }),
    ...Object.entries(reasoning).map(([invocation, text]) => {
      const seen = order[`reasoning:${invocation}`];
      return {
        item: { kind: "reasoning" as const, key: `reasoning:${invocation}`, invocation, text },
        seen,
        at: seen?.at ?? Number.MAX_SAFE_INTEGER,
      };
    }),
  ];
  entries.sort((a, b) => {
    if (a.seen && b.seen && a.seen.sequence !== b.seen.sequence)
      return a.seen.sequence < b.seen.sequence ? -1 : 1;
    return a.at - b.at || a.item.key.localeCompare(b.item.key);
  });
  const items: TimelineItem[] = [];
  for (const entry of entries) {
    const previous = items.at(-1);
    if (entry.tool && previous?.kind === "tools") previous.tools.push(entry.tool);
    else items.push(entry.item);
  }
  return items;
}
