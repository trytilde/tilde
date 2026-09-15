import type { Tool as ChannelTool } from "@trytilde/sdk";
import { jsonSchema, tool, type ToolSet } from "ai";

export type ConvertToAiSdkToolsOptions = {
  /** Override provider descriptions without modifying their schemas or authorization. */
  instructions?: Record<string, string>;
  /** Namespace tool names when combining multiple channel collections. */
  prefix?: string;
};

/** Preserve provider schemas, execution IDs and cancellation when exposing channel tools to Vercel. */
export function convertToAiSdkTools(
  channels: Readonly<Record<string, ChannelTool | undefined>>,
  options: ConvertToAiSdkToolsOptions = {},
): ToolSet {
  const result: ToolSet = Object.create(null);
  for (const [name, channel] of Object.entries(channels)) {
    if (!channel) continue;
    const key = `${options.prefix ?? ""}${name}`.replace(/[^a-zA-Z0-9_-]/g, "_");
    if (!key || key.length > 64 || Object.hasOwn(result, key))
      throw new Error("Conflicting or invalid model tool names");
    result[key] = tool({
      description:
        options.instructions && Object.hasOwn(options.instructions, name)
          ? options.instructions[name]
          : channel.description,
      inputSchema: jsonSchema(channel.inputSchema),
      execute: async (input, { toolCallId, abortSignal }) => {
        abortSignal?.throwIfAborted();
        return channel.execute(input, { toolCallId });
      },
    });
  }
  return result;
}
