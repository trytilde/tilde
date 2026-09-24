import { randomUUID } from "node:crypto";
import type { Tool as ChannelTool } from "@trytilde/sdk";
import { createTool, type Tool } from "@mastra/core/tools";
import type { JSONSchema7 } from "ai";

export type ConvertToMastraToolsOptions = {
  /** Override provider descriptions without modifying their schemas or authorization. */
  instructions?: Record<string, string>;
  /** Namespace tool names when combining multiple channel collections. */
  prefix?: string;
};

/**
 * Preserve provider schemas, execution IDs and cancellation when exposing channel tools to Mastra.
 * `createTool` accepts a plain JSON Schema as `inputSchema` (Mastra wraps it in an Ajv-backed
 * Standard Schema), so the provider schema is passed through unchanged.
 */
export function convertToMastraTools(
  channels: Readonly<Record<string, ChannelTool | undefined>>,
  options: ConvertToMastraToolsOptions = {},
): Record<string, Tool> {
  const result: Record<string, Tool> = Object.create(null);
  for (const [name, channel] of Object.entries(channels)) {
    if (!channel) continue;
    const key = `${options.prefix ?? ""}${name}`.replace(/[^a-zA-Z0-9_-]/g, "_");
    if (!key || key.length > 64 || Object.hasOwn(result, key))
      throw new Error("Conflicting or invalid model tool names");
    result[key] = createTool({
      id: key,
      description:
        options.instructions && Object.hasOwn(options.instructions, name)
          ? options.instructions[name]
          : channel.description,
      inputSchema: channel.inputSchema as JSONSchema7,
      // Inside an agent run Mastra exposes the model's tool-call ID as `agent.toolCallId`.
      // A direct `tool.execute(input)` outside an agent loop has no model ID and gets a fresh UUID.
      execute: async (input, { abortSignal, agent }) => {
        abortSignal?.throwIfAborted();
        return channel.execute(input, { toolCallId: agent?.toolCallId || randomUUID() });
      },
    });
  }
  return result;
}
