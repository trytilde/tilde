import type { Tool as ChannelTool } from "@trytilde/sdk";
import { tool, type Tool, type ToolInputParameters } from "@openai/agents";

export type ConvertToOpenAIAgentsToolsOptions = {
  /** Override provider descriptions without modifying their schemas or authorization. */
  instructions?: Record<string, string>;
  /** Namespace tool names when combining multiple channel collections. */
  prefix?: string;
};
/** Non-strict JSON schema parameters are handed to the model unchanged; strict mode rewrites them. */
type ProviderSchema = Extract<ToolInputParameters, { additionalProperties: true }>;

/** Preserve provider schemas, execution IDs and cancellation when exposing channel tools to OpenAI Agents. */
export function convertToOpenAIAgentsTools(
  channels: Readonly<Record<string, ChannelTool | undefined>>,
  options: ConvertToOpenAIAgentsToolsOptions = {},
): Tool[] {
  const result: Tool[] = [];
  const names = new Set<string>();
  for (const [name, channel] of Object.entries(channels)) {
    if (!channel) continue;
    // OpenAI Agents rewrites every non-alphanumeric character, including "-", to "_".
    const key = `${options.prefix ?? ""}${name}`.replace(/[^a-zA-Z0-9_]/g, "_");
    if (!key || key.length > 64 || names.has(key))
      throw new Error("Conflicting or invalid model tool names");
    names.add(key);
    result.push(
      tool({
        name: key,
        description:
          options.instructions && Object.hasOwn(options.instructions, name)
            ? options.instructions[name]
            : channel.description,
        parameters: channel.inputSchema as ProviderSchema,
        strict: false,
        // The runner supplies the model's call id as details.toolCall.callId and the run signal.
        execute: async (input, _runContext, details) => {
          details?.signal?.throwIfAborted();
          const toolCallId = details?.toolCall?.callId;
          if (!toolCallId) throw new Error("Tool call ID is required");
          return channel.execute(input, { toolCallId });
        },
      }),
    );
  }
  return result;
}
