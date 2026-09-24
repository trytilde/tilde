import type {
  AgentContext,
  ContextMessage,
  ConversationMessage,
  GoalMessage,
  ObjectiveMessage,
  TaskMessage,
} from "@trytilde/sdk";
import { protocol, type AgentInputItem } from "@openai/agents";
import { convertAttachment, type AttachmentHandler } from "./attachments.js";

type Converted = AgentInputItem | null | Promise<AgentInputItem | null>;
export type MessageHandlers = {
  message?: (message: ConversationMessage) => Converted;
  objective?: (message: ObjectiveMessage) => Converted;
  goal?: (message: GoalMessage) => Converted;
  task?: (message: TaskMessage) => Converted;
};
export type ConvertToOpenAIAgentsMessagesOptions = {
  messages: Iterable<ContextMessage>;
  context?: Pick<AgentContext, "attachments" | "cacheConvertedMessages">;
  /** An explicit handler takes precedence over both cached and default conversion. Null omits an item. */
  onMessage?: MessageHandlers;
  onAttachment?: AttachmentHandler;
};
type CacheEntry = Parameters<AgentContext["cacheConvertedMessages"]>[0]["messages"][number];
type MessageItem = protocol.UserMessageItem | protocol.AssistantMessageItem;

/**
 * Convert typed SDK context into OpenAI Agents input items, ready for `run(agent, items)`.
 *
 * Returned items carry no `id`: the OpenAI Responses model forwards item ids verbatim and the
 * API rejects ids it did not issue. The Tilde message id is kept on the cached representation
 * instead, where hydration verifies it against the canonical message.
 */
export async function convertToOpenAIAgentsMessages(
  options: ConvertToOpenAIAgentsMessagesOptions,
): Promise<AgentInputItem[]> {
  const output: AgentInputItem[] = [];
  const cache: CacheEntry[] = [];
  let bytes = 0;
  const flush = async () => {
    if (cache.length && options.context)
      await options.context.cacheConvertedMessages({ messages: cache.splice(0) });
    bytes = 0;
  };
  for (const item of options.messages) {
    if (
      item.type === "message" &&
      item.message.status !== "complete" &&
      !options.onMessage?.message
    )
      continue;
    const handler = options.onMessage;
    let converted: AgentInputItem | null;
    let hydrated = false;
    switch (item.type) {
      case "objective":
        converted = handler?.objective ? await handler.objective(item) : userText(item.objective);
        break;
      case "goal":
        converted = handler?.goal
          ? await handler.goal(item)
          : userText(`Goal (${item.goal.status}): ${item.goal.objective}`);
        break;
      case "task":
        converted = handler?.task
          ? await handler.task(item)
          : userText(
              `Task (${item.task.status}): ${item.task.title}${item.task.blockedReason ? `\nBlocked: ${item.task.blockedReason}` : ""}`,
            );
        break;
      case "message": {
        if (handler?.message) {
          converted = await handler.message(item);
          break;
        }
        // File bytes are hydrated afresh through the scoped SDK, never replayed from cached URLs.
        converted = item.message.attachments.length === 0 ? hydrate(item) : null;
        hydrated = converted !== null;
        if (!converted) converted = await render(item, options);
        break;
      }
    }
    if (!converted) continue;
    output.push(converted);
    // Only canonical messages can be cached by the runtime. Work state is always current,
    // and hydrated files must not duplicate large/sensitive attachment bytes in the cache.
    if (
      options.context &&
      item.type === "message" &&
      item.message.status === "complete" &&
      !hydrated &&
      item.message.attachments.length === 0
    ) {
      const serialized = JSON.stringify({ id: item.id, ...converted });
      const size = Buffer.byteLength(serialized);
      if (size > 1024 * 1024) continue;
      if (cache.length === 100 || bytes + size > 1024 * 1024) await flush();
      cache.push({
        chatMessageId: item.id,
        message: JSON.parse(serialized) as CacheEntry["message"],
      });
      bytes += size;
    }
  }
  await flush();
  return output;
}

/** One user message item holding a single input_text part. */
function userText(text: string): protocol.UserMessageItem {
  return { type: "message", role: "user", content: [{ type: "input_text", text }] };
}

/** Render a conversation message from its canonical text and attachments. */
async function render(
  item: ConversationMessage,
  options: ConvertToOpenAIAgentsMessagesOptions,
): Promise<MessageItem | null> {
  const texts: string[] = [];
  if (item.message.subject) texts.push(`Subject: ${item.message.subject}`);
  if (item.message.text) texts.push(item.message.text);
  if (item.role === "assistant") {
    // Assistant items only carry model output; the agent's own attachments are named, not embedded.
    for (const attachment of item.message.attachments)
      texts.push(`Attached file: ${attachment.filename} (${attachment.mediaType})`);
    if (!texts.length) return null;
    return {
      type: "message",
      role: "assistant",
      status: "completed",
      content: texts.map((text) => ({ type: "output_text", text })),
    };
  }
  const content: protocol.UserContent[] = texts.map((text) => ({ type: "input_text", text }));
  for (const attachment of item.message.attachments) {
    const part = await (options.onAttachment ?? convertAttachment)({
      message: item,
      attachment,
      download: () => {
        if (!options.context)
          throw new Error(
            "Attachment conversion requires context or a custom onAttachment handler",
          );
        return options.context.attachments.download(attachment.id);
      },
    });
    if (part) content.push(...(Array.isArray(part) ? part : [part]));
  }
  return content.length ? { type: "message", role: "user", content } : null;
}

/** Reuse a cached representation only when its id, role and text-only content match. */
function hydrate(item: ConversationMessage): MessageItem | null {
  const cached = item.cachedAgentRepresentation;
  if (!cached || typeof cached !== "object" || Array.isArray(cached)) return null;
  if (cached.id !== item.id || cached.role !== item.role) return null;
  const schema = item.role === "user" ? protocol.UserMessageItem : protocol.AssistantMessageItem;
  const parsed = schema.safeParse(cached);
  if (!parsed.success) return null;
  const { id: _id, ...message } = parsed.data;
  const parts: { type: string }[] = typeof message.content === "string" ? [] : message.content;
  if (parts.some((part) => part.type === "input_image" || part.type === "input_file")) return null;
  return message;
}
