import { randomUUID } from "node:crypto";
import {
  auditToolCall,
  modelToolName,
  type AgentContext,
  type BundledTool,
  type BundledToolOptions,
  type DiscoveredTool,
  type Tool as ChannelTool,
} from "@trytilde/sdk";
import { ToolMessage, type ToolCall } from "@langchain/core/messages";
import { tool, type StructuredToolInterface, type ToolRunnableConfig } from "@langchain/core/tools";
import { toJsonSchema } from "@langchain/core/utils/json_schema";

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
    const key = modelToolName(`${options.prefix ?? ""}${name}`);
    if (!key || names.has(key)) throw new Error("Conflicting or invalid model tool names");
    names.add(key);
    result.push(
      tool(
        async (input: unknown, config: ToolRunnableConfig) => {
          // tool() only listens for later aborts; reject an already-cancelled invocation up front.
          config.signal?.throwIfAborted();
          // LangGraph's ToolNode and langchain's createAgent invoke with the model's ToolCall, which
          // BaseTool.invoke places on config.toolCall. Direct calls without one get a fresh ID.
          // A routed tools.execute hands this config to the bundled tool it names.
          return channel.execute(input, {
            toolCallId: config.toolCall?.id ?? randomUUID(),
            frameworkContext: config,
          });
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

/**
 * The agent's tools for `createAgent`, `createReactAgent` or `bindTools` in one list: the current
 * channel's tools, its other Tilde tools (`ctx.agentTools`) and `tools`, its own LangChain tools.
 * Those are published to Tilde for this invocation, so `tools.search`/`tools.schemas` describe
 * them and a `tools.execute` naming one runs it here; every call is audited once, with the
 * model's call ID. Tilde's summary, display and annotations come from a tool's `metadata.tilde`,
 * overridden by `options[name]`. LangChain tools declare no output schema.
 *
 * Caveats: input that fails the tool's schema is audited as a failed call. A returned
 * `ToolMessage` with status "error" is audited as failed. Tools that read LangGraph-injected
 * state or runtime can only run inside the graph, so a `tools.execute` naming one runs it
 * without that state.
 */
export function withTildeTools(
  ctx: AgentContext,
  tools: StructuredToolInterface[],
  options: BundledToolOptions = {},
): Promise<StructuredToolInterface[]> {
  return invocationTools(ctx, tools, options);
}

/** @internal `withTildeTools` with options for the channel's tools, as the invocation helper uses it. */
export async function invocationTools(
  ctx: AgentContext,
  tools: StructuredToolInterface[],
  options: BundledToolOptions,
  channel: ConvertToLangChainToolsOptions = {},
): Promise<StructuredToolInterface[]> {
  const result = convertToLangChainTools({ ...ctx.channel.current, ...ctx.agentTools }, channel);
  const names = new Set(result.map((t) => t.name));
  const bundled: BundledTool[] = [];
  for (const native of tools) {
    const name = native.name;
    if (!/^[a-zA-Z0-9_-]{1,64}$/.test(name) || names.has(name))
      throw new Error(`Tool ${name} is not a valid model tool name or conflicts with another tool`);
    names.add(name);
    const described = describeTool(native, options);
    const audited = (input: unknown, toolCallId: string, run: () => Promise<unknown>) =>
      auditToolCall(
        ctx,
        { name, toolCallId, input, summary: described.summary, display: described.display },
        run,
        (output) => {
          if (!ToolMessage.isInstance(output)) return { output };
          if (output.status === "error")
            return {
              error:
                typeof output.content === "string"
                  ? output.content
                  : JSON.stringify(output.content),
            };
          return {
            output:
              output.artifact === undefined
                ? output.content
                : { content: output.content, artifact: output.artifact },
          };
        },
      );
    // A prototype clone keeps the tool's class and fields; only invoke is replaced, so input
    // parsing errors thrown before the tool body are audited too.
    const wrapped: StructuredToolInterface = Object.create(native);
    wrapped.invoke = (async (input: unknown, config?: ToolRunnableConfig) => {
      const call = isToolCall(input) ? input : config?.toolCall;
      return audited(call ? call.args : input, call?.id ?? randomUUID(), () =>
        native.invoke(input as never, config),
      );
    }) as StructuredToolInterface["invoke"];
    result.push(wrapped);
    bundled.push({
      ...described,
      // Invoked without the outer tools.execute call, the tool returns its raw output rather
      // than a ToolMessage addressed to that call.
      execute: (input, { toolCallId, frameworkContext }) =>
        audited(input, toolCallId, () =>
          native.invoke(input as never, {
            ...(frameworkContext as ToolRunnableConfig | undefined),
            toolCall: undefined,
          }),
        ),
    });
  }
  // Published even when empty, so tools removed since the last call stop running here.
  await ctx.setBundledTools(bundled);
  return result;
}

/** @internal How Tilde describes a native tool, for `withTildeTools` and `tilde deploy` alike. */
export function describeTool(
  native: StructuredToolInterface,
  options: BundledToolOptions,
): Omit<DiscoveredTool, "origin"> {
  const metadata = (native as { metadata?: Record<string, unknown> }).metadata;
  return {
    name: native.name,
    description: native.description,
    ...(metadata?.tilde as BundledToolOptions[string]),
    ...options[native.name],
    inputSchema: toJsonSchema(native.schema) as Record<string, unknown>,
  };
}

function isToolCall(input: unknown): input is ToolCall {
  return (input as ToolCall | null)?.type === "tool_call";
}
