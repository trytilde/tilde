import {
  auditToolCall,
  modelToolName,
  type AgentContext,
  type BundledTool,
  type BundledToolOptions,
  type DiscoveredTool,
  type Tool as ChannelTool,
} from "@trytilde/sdk";
import { asSchema, jsonSchema, tool, type ToolExecutionOptions, type ToolSet } from "ai";

type NativeTool = ToolSet[string];

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
    const key = modelToolName(`${options.prefix ?? ""}${name}`);
    if (!key || Object.hasOwn(result, key))
      throw new Error("Conflicting or invalid model tool names");
    result[key] = tool({
      description:
        options.instructions && Object.hasOwn(options.instructions, name)
          ? options.instructions[name]
          : channel.description,
      inputSchema: jsonSchema(channel.inputSchema),
      execute: async (input, execution) => {
        execution.abortSignal?.throwIfAborted();
        // A routed tools.execute hands these options to the bundled tool it names.
        return channel.execute(input, {
          toolCallId: execution.toolCallId,
          frameworkContext: execution,
        });
      },
    });
  }
  return result;
}

/**
 * The agent's tools for `generateText`/`streamText` in one `ToolSet`: the current channel's tools,
 * its other Tilde tools (`ctx.agentTools`) and `tools`, its own AI SDK tools. Those are published
 * to Tilde for this invocation, so `tools.search`/`tools.schemas` describe them and a
 * `tools.execute` naming one runs it here; every call is audited once, with the model's call ID.
 * Tilde's summary, display and annotations come from a tool's `metadata.tilde` (never sent to the
 * model), overridden by `options[name]`.
 *
 * Caveats: provider-defined tools (`type: "provider"`) and tools without `execute` pass through
 * unpublished and unaudited, since this process does not run them. An `execute` returning an
 * AsyncIterable is drained: the framework and the audit see only its final value, not
 * preliminary results.
 */
export function withTildeTools(
  ctx: AgentContext,
  tools: ToolSet,
  options: BundledToolOptions = {},
): Promise<ToolSet> {
  return invocationTools(ctx, tools, options);
}

/** @internal `withTildeTools` with options for the channel's tools, as `prepareTildeCall` uses it. */
export async function invocationTools(
  ctx: AgentContext,
  tools: ToolSet,
  options: BundledToolOptions,
  channel: ConvertToAiSdkToolsOptions = {},
): Promise<ToolSet> {
  const result = convertToAiSdkTools({ ...ctx.channel.current, ...ctx.agentTools }, channel);
  const bundled: BundledTool[] = [];
  for (const [name, native] of Object.entries(tools)) {
    if (!/^[a-zA-Z0-9_-]{1,64}$/.test(name) || Object.hasOwn(result, name))
      throw new Error(`Tool ${name} is not a valid model tool name or conflicts with another tool`);
    const described = await describeTool(name, native, options);
    const execute = native.execute;
    if (!described || !execute) {
      result[name] = native;
      continue;
    }
    const run = (input: unknown, execution: ToolExecutionOptions) =>
      auditToolCall(
        ctx,
        {
          name,
          toolCallId: execution.toolCallId,
          input,
          summary: described.summary,
          display: described.display,
        },
        async () => {
          const output = await execute(input, execution);
          if (!isAsyncIterable(output)) return output;
          let last: unknown;
          for await (const value of output) last = value;
          return last;
        },
      );
    result[name] = { ...native, execute: run };
    const inputSchema = asSchema(native.inputSchema);
    bundled.push({
      ...described,
      // tools.execute input skips the AI SDK's own validation, so validate it as the SDK would.
      execute: async (input, { toolCallId, frameworkContext }) => {
        const checked = await inputSchema.validate?.(input);
        if (checked && !checked.success) throw checked.error;
        return run(checked ? checked.value : input, {
          messages: [],
          abortSignal: ctx.signal,
          ...(frameworkContext as Partial<ToolExecutionOptions> | undefined),
          toolCallId,
        });
      },
    });
  }
  // Published even when empty, so tools removed since the last call stop running here.
  await ctx.setBundledTools(bundled);
  return result;
}

/**
 * @internal How Tilde describes a native tool, for `withTildeTools` and `tilde deploy` alike.
 * Undefined for tools this process does not run: provider-defined tools and tools without `execute`.
 */
export async function describeTool(
  name: string,
  native: NativeTool,
  options: BundledToolOptions,
): Promise<Omit<DiscoveredTool, "origin"> | undefined> {
  if (native.type === "provider" || !native.execute) return undefined;
  return {
    name,
    description: native.description ?? "",
    ...(native.metadata?.tilde as BundledToolOptions[string]),
    ...options[name],
    inputSchema: (await asSchema(native.inputSchema).jsonSchema) as Record<string, unknown>,
    // asSchema(undefined) is an empty object schema, not "no output schema".
    outputSchema: native.outputSchema
      ? ((await asSchema(native.outputSchema).jsonSchema) as Record<string, unknown>)
      : undefined,
  };
}

function isAsyncIterable(value: unknown): value is AsyncIterable<unknown> {
  return (
    value != null && typeof (value as AsyncIterable<unknown>)[Symbol.asyncIterator] === "function"
  );
}
