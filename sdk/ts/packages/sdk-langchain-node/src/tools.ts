import { randomUUID } from "node:crypto";
import type { Tool as ChannelTool } from "@trytilde/sdk";
import { tool, type StructuredToolInterface, type ToolRunnableConfig } from "@langchain/core/tools";

export type ConvertToLangChainToolsOptions = {
  /** Override provider descriptions without modifying their schemas or authorization. */
  instructions?: Record<string, string>;
  /** Namespace tool names when combining multiple channel collections. */
  prefix?: string;
};

/** Preserve provider schemas, execution IDs and cancellation when exposing channel tools to LangChain. */
export function convertToLangChainTools(
  channels: Readonly<Record<string, ChannelTool | undefined>>,
  options: ConvertToLangChainToolsOptions = {},
): StructuredToolInterface[] {
  const result: StructuredToolInterface[] = [];
  const names = new Set<string>();
  for (const [name, channel] of Object.entries(channels)) {
    if (!channel) continue;
    const key = `${options.prefix ?? ""}${name}`.replace(/[^a-zA-Z0-9_-]/g, "_");
    if (!key || key.length > 64 || names.has(key))
      throw new Error("Conflicting or invalid model tool names");
    names.add(key);
    result.push(
      tool(
        async (input: unknown, config: ToolRunnableConfig) => {
          // tool() only listens for later aborts; reject an already-cancelled invocation up front.
          config.signal?.throwIfAborted();
          // LangGraph's ToolNode and langchain's createAgent invoke with the model's ToolCall, which
          // BaseTool.invoke places on config.toolCall. Direct calls without one get a fresh ID.
          return channel.execute(input, { toolCallId: config.toolCall?.id ?? randomUUID() });
        },
        {
          name: key,
          description:
            options.instructions && Object.hasOwn(options.instructions, name)
              ? options.instructions[name]
              : channel.description,
          schema: channel.inputSchema,
        },
      ),
    );
  }
  return result;
}
