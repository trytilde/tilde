import type { JsonValue, Resumed } from "@trytilde/sdk";
import type { ModelMessage } from "ai";

/**
 * Model messages as the JSON a run suspends with. Binary file and image data becomes base64,
 * which the AI SDK accepts back as data; URLs become their text.
 */
export function toJson(messages: ModelMessage[]): JsonValue {
  return JSON.parse(
    JSON.stringify(messages, (_key, value: unknown) => {
      if (value instanceof Uint8Array) return Buffer.from(value).toString("base64");
      // Buffers serialize themselves before the replacer sees them.
      const buffer = value as { type?: unknown; data?: unknown } | null;
      if (buffer?.type === "Buffer" && Array.isArray(buffer.data))
        return Buffer.from(buffer.data as number[]).toString("base64");
      return value;
    }),
  ) as JsonValue;
}

/** The saved conversation, each pending tool result replaced by the call's outcome. */
export function restore(resumed: Resumed): ModelMessage[] {
  const messages = resumed.state as unknown as ModelMessage[];
  return messages.map((message) => {
    if (message.role !== "tool") return message;
    return {
      ...message,
      content: message.content.map((part) => {
        const outcome = part.type === "tool-result" && resumed.outcomes.get(part.toolCallId);
        if (!outcome || part.type !== "tool-result") return part;
        return {
          ...part,
          output:
            "output" in outcome
              ? { type: "json" as const, value: outcome.output }
              : { type: "error-text" as const, value: outcome.error },
        };
      }),
    };
  });
}
