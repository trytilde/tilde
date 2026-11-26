import type { JsonValue, Resumed } from "@trytilde/sdk";
import type { AgentInputItem } from "@openai/agents";

/** The run's input items as the JSON a run suspends with (they are protocol JSON already). */
export function toJson(items: AgentInputItem[]): JsonValue {
  return JSON.parse(JSON.stringify(items)) as JsonValue;
}

/** The saved items, each pending function call result replaced by the call's outcome. */
export function restore(resumed: Resumed): AgentInputItem[] {
  return (resumed.state as unknown as AgentInputItem[]).map((item) => {
    if (item.type !== "function_call_result") return item;
    const outcome = resumed.outcomes.get(item.callId);
    if (!outcome) return item;
    return {
      ...item,
      status: "completed" as const,
      output: {
        type: "text" as const,
        text: "output" in outcome ? JSON.stringify(outcome.output) : `Error: ${outcome.error}`,
      },
    };
  });
}
