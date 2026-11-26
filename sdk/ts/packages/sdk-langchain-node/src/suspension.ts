import type { JsonValue, Resumed } from "@trytilde/sdk";
import {
  ToolMessage,
  mapChatMessagesToStoredMessages,
  mapStoredMessagesToChatMessages,
  type BaseMessage,
  type StoredMessage,
} from "@langchain/core/messages";

/** LangGraph's `REMOVE_ALL_MESSAGES`: a `RemoveMessage` with this ID clears the state's messages. */
export const REMOVE_ALL_MESSAGES = "__remove_all__";

/** The agent state's messages as the JSON a run suspends with. */
export function toJson(messages: BaseMessage[]): JsonValue {
  return mapChatMessagesToStoredMessages(messages) as unknown as JsonValue;
}

/** The saved messages, each pending tool message replaced by the call's outcome. */
export function restore(resumed: Resumed): BaseMessage[] {
  return mapStoredMessagesToChatMessages(resumed.state as unknown as StoredMessage[]).map(
    (message) => {
      if (!ToolMessage.isInstance(message)) return message;
      const outcome = resumed.outcomes.get(message.tool_call_id);
      if (!outcome) return message;
      return new ToolMessage({
        id: message.id,
        name: message.name,
        tool_call_id: message.tool_call_id,
        ...("output" in outcome
          ? { content: JSON.stringify(outcome.output) }
          : { content: outcome.error, status: "error" as const }),
      });
    },
  );
}
