import type { JsonValue, Resumed } from "@trytilde/sdk";
import type { MastraDBMessage } from "@mastra/core/agent";

/** Mastra's message list as the JSON a run suspends with (dates become ISO text). */
export function toJson(messages: MastraDBMessage[]): JsonValue {
  return JSON.parse(JSON.stringify(messages)) as JsonValue;
}

type Invocation = { toolCallId: string; state: string; result?: unknown };

/**
 * The saved message list, each pending tool result replaced by the call's outcome (a failed
 * call's as `{ error }`), in the parts and in the legacy `toolInvocations` Mastra keeps beside them.
 */
export function restore(resumed: Resumed): MastraDBMessage[] {
  const settle = (invocation: Invocation) => {
    const outcome = resumed.outcomes.get(invocation.toolCallId);
    if (!outcome) return invocation;
    return {
      ...invocation,
      state: "result",
      result: "output" in outcome ? outcome.output : { error: outcome.error },
    };
  };
  const messages = resumed.state as unknown as (MastraDBMessage & { createdAt: string })[];
  return messages.map((message) => {
    const content = message.content as {
      parts?: { type: string; toolInvocation?: Invocation }[];
      toolInvocations?: Invocation[];
    };
    return {
      ...message,
      createdAt: new Date(message.createdAt),
      content: {
        ...message.content,
        parts: content.parts?.map((part) =>
          part.type === "tool-invocation" && part.toolInvocation
            ? { ...part, toolInvocation: settle(part.toolInvocation) }
            : part,
        ),
        ...(content.toolInvocations && { toolInvocations: content.toolInvocations.map(settle) }),
      },
    } as MastraDBMessage;
  });
}
