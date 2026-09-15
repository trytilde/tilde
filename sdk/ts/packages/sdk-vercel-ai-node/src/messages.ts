import type {
  AgentContext,
  ContextMessage,
  ConversationMessage,
  GoalMessage,
  ObjectiveMessage,
  TaskMessage,
} from "@trytilde/sdk";
import { validateUIMessages, type UIMessage } from "ai";
import { convertAttachment, type AttachmentHandler } from "./attachments.js";

type Converted = UIMessage | null | Promise<UIMessage | null>;
export type MessageHandlers = {
  message?: (message: ConversationMessage) => Converted;
  objective?: (message: ObjectiveMessage) => Converted;
  goal?: (message: GoalMessage) => Converted;
  task?: (message: TaskMessage) => Converted;
};
export type ConvertToAiSdkMessagesOptions = {
  messages: Iterable<ContextMessage>;
  context?: Pick<AgentContext, "attachments" | "cacheConvertedMessages">;
  /** An explicit handler takes precedence over both cached and default conversion. Null omits an item. */
  onMessage?: MessageHandlers;
  onAttachment?: AttachmentHandler;
};
type CacheEntry = Parameters<AgentContext["cacheConvertedMessages"]>[0]["messages"][number];

/** Convert typed SDK context into AI SDK UI messages, ready for convertToModelMessages. */
export async function convertToAiSdkMessages(
  options: ConvertToAiSdkMessagesOptions,
): Promise<UIMessage[]> {
  const output: UIMessage[] = [];
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
    let converted: UIMessage | null;
    let hydrated = false;
    switch (item.type) {
      case "objective":
        converted = handler?.objective
          ? await handler.objective(item)
          : textMessage(item.id, "user", item.objective);
        break;
      case "goal":
        converted = handler?.goal
          ? await handler.goal(item)
          : textMessage(item.id, "user", `Goal (${item.goal.status}): ${item.goal.objective}`);
        break;
      case "task":
        converted = handler?.task
          ? await handler.task(item)
          : textMessage(
              item.id,
              "user",
              `Task (${item.task.status}): ${item.task.title}${item.task.blockedReason ? `\nBlocked: ${item.task.blockedReason}` : ""}`,
            );
        break;
      case "message": {
        if (handler?.message) {
          converted = await handler.message(item);
          break;
        }
        // File bytes are hydrated afresh through the scoped SDK, never replayed from cached URLs.
        converted = item.message.attachments.length === 0 ? await hydrate(item) : null;
        hydrated = converted !== null;
        if (!converted) {
          const parts: UIMessage["parts"] = [];
          if (item.message.subject)
            parts.push({ type: "text", text: `Subject: ${item.message.subject}` });
          if (item.message.text) parts.push({ type: "text", text: item.message.text });
          for (const attachment of item.message.attachments) {
            const convertedPart = await (options.onAttachment ?? convertAttachment)({
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
            if (convertedPart)
              parts.push(...(Array.isArray(convertedPart) ? convertedPart : [convertedPart]));
          }
          converted = parts.length ? { id: item.id, role: item.role, parts } : null;
        }
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
      const serialized = JSON.stringify(converted);
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

function textMessage(id: string, role: UIMessage["role"], text: string): UIMessage {
  return { id, role, parts: [{ type: "text", text }] };
}
async function hydrate(item: ConversationMessage): Promise<UIMessage | null> {
  if (!item.cachedAgentRepresentation) return null;
  try {
    const [message] = await validateUIMessages({ messages: [item.cachedAgentRepresentation] });
    if (
      message.id !== item.id ||
      message.role !== item.role ||
      message.parts.some((part) => part.type === "file")
    )
      return null;
    return message;
  } catch {
    return null;
  }
}
