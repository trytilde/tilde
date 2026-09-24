import type {
  AgentContext,
  ContextMessage,
  ConversationMessage,
  GoalMessage,
  ObjectiveMessage,
  TaskMessage,
} from "@trytilde/sdk";
import { AIMessage, HumanMessage, type BaseMessage } from "@langchain/core/messages";
import { convertAttachment, type AttachmentBlock, type AttachmentHandler } from "./attachments.js";

type Converted = BaseMessage | null | Promise<BaseMessage | null>;
export type MessageHandlers = {
  message?: (message: ConversationMessage) => Converted;
  objective?: (message: ObjectiveMessage) => Converted;
  goal?: (message: GoalMessage) => Converted;
  task?: (message: TaskMessage) => Converted;
};
export type ConvertToLangChainMessagesOptions = {
  messages: Iterable<ContextMessage>;
  context?: Pick<AgentContext, "attachments" | "cacheConvertedMessages">;
  /** An explicit handler takes precedence over both cached and default conversion. Null omits an item. */
  onMessage?: MessageHandlers;
  onAttachment?: AttachmentHandler;
};
type CacheEntry = Parameters<AgentContext["cacheConvertedMessages"]>[0]["messages"][number];
type Role = ConversationMessage["role"];
type TextBlock = { type: "text"; text: string };
/** The cached form is plain JSON rather than a serialized LangChain class: id, role and text-only content. */
type CachedMessage = { id: string; role: Role; content: string | TextBlock[] };

/** Convert typed SDK context into LangChain messages, ready for a chat model or LangGraph agent. */
export async function convertToLangChainMessages(
  options: ConvertToLangChainMessagesOptions,
): Promise<BaseMessage[]> {
  const output: BaseMessage[] = [];
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
    let converted: BaseMessage | null;
    let cacheable: CachedMessage | undefined;
    switch (item.type) {
      case "objective":
        converted = handler?.objective
          ? await handler.objective(item)
          : message({ id: item.id, role: "user", content: item.objective });
        break;
      case "goal":
        converted = handler?.goal
          ? await handler.goal(item)
          : message({
              id: item.id,
              role: "user",
              content: `Goal (${item.goal.status}): ${item.goal.objective}`,
            });
        break;
      case "task":
        converted = handler?.task
          ? await handler.task(item)
          : message({
              id: item.id,
              role: "user",
              content: `Task (${item.task.status}): ${item.task.title}${item.task.blockedReason ? `\nBlocked: ${item.task.blockedReason}` : ""}`,
            });
        break;
      case "message": {
        if (handler?.message) {
          converted = await handler.message(item);
          break;
        }
        // File bytes are hydrated afresh through the scoped SDK, never replayed from the cache.
        const cached = item.message.attachments.length === 0 ? hydrate(item) : undefined;
        if (cached) {
          converted = message(cached);
          break;
        }
        const blocks: AttachmentBlock[] = [];
        if (item.message.subject)
          blocks.push({ type: "text", text: `Subject: ${item.message.subject}` });
        if (item.message.text) blocks.push({ type: "text", text: item.message.text });
        for (const attachment of item.message.attachments) {
          const convertedBlock = await (options.onAttachment ?? convertAttachment)({
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
          if (convertedBlock)
            blocks.push(...(Array.isArray(convertedBlock) ? convertedBlock : [convertedBlock]));
        }
        if (!blocks.length) {
          converted = null;
          break;
        }
        const content = blocks.length === 1 && isTextBlock(blocks[0]) ? blocks[0].text : blocks;
        converted = message({ id: item.id, role: item.role, content });
        // Only canonical messages can be cached by the runtime. Work state is always current,
        // and hydrated files must not duplicate large/sensitive attachment bytes in the cache.
        if (item.message.status === "complete" && item.message.attachments.length === 0)
          cacheable = { id: item.id, role: item.role, content: content as string | TextBlock[] };
        break;
      }
    }
    if (!converted) continue;
    output.push(converted);
    if (options.context && cacheable) {
      const serialized = JSON.stringify(cacheable);
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

function message(input: { id: string; role: Role; content: string | AttachmentBlock[] }) {
  const { id, content } = input;
  return input.role === "assistant"
    ? new AIMessage({ id, content })
    : new HumanMessage({ id, content });
}
function isTextBlock(block: unknown): block is TextBlock {
  return (
    typeof block === "object" &&
    block !== null &&
    (block as TextBlock).type === "text" &&
    typeof (block as TextBlock).text === "string"
  );
}
/** Accept a cached rendering only for this message's identity and role, and never with file data. */
function hydrate(item: ConversationMessage): CachedMessage | undefined {
  const cached = item.cachedAgentRepresentation;
  if (typeof cached !== "object" || cached === null || Array.isArray(cached)) return undefined;
  const { id, role, content } = cached as Partial<CachedMessage>;
  if (id !== item.id || role !== item.role) return undefined;
  if (typeof content === "string") return { id, role, content };
  if (Array.isArray(content) && content.length && content.every(isTextBlock))
    return { id, role, content };
  return undefined;
}
