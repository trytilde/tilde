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
import {
  RunContext,
  tool,
  type FunctionTool,
  type Tool,
  type ToolInputParameters,
} from "@openai/agents";

export type ConvertToOpenAIAgentsToolsOptions = {
  /** Override provider descriptions without modifying their schemas or authorization. */
  instructions?: Record<string, string>;
  /** Namespace tool names when combining multiple channel collections. */
  prefix?: string;
};
type RoutedContext = { runContext?: RunContext<unknown>; signal?: AbortSignal };
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
    const key = modelToolName(`${options.prefix ?? ""}${name}`, /[^a-zA-Z0-9_]/g);
    if (!key || names.has(key)) throw new Error("Conflicting or invalid model tool names");
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
        execute: async (input, runContext, details) => {
          details?.signal?.throwIfAborted();
          const toolCallId = details?.toolCall?.callId;
          if (!toolCallId) throw new Error("Tool call ID is required");
          // A routed tools.execute hands the run context to the bundled tool it names.
          const frameworkContext: RoutedContext = { runContext, signal: details?.signal };
          return channel.execute(input, { toolCallId, frameworkContext });
        },
      }),
    );
  }
  return result;
}

/**
 * The agent's tools for an OpenAI Agents `Agent` in one list: the current channel's tools, its
 * other Tilde tools (`ctx.agentTools`) and `tools`, its own `tool()` function tools. Those are
 * published to Tilde for this invocation, so `tools.search`/`tools.schemas` describe them and a
 * `tools.execute` naming one runs it here with the outer call's `RunContext`; every call is
 * audited once, with the model's call ID. Function tools carry no Tilde metadata, so summary,
 * display and annotations come from `options[name]`.
 *
 * Caveats: a tool's `errorFunction` (by default, for tools without an output schema) turns a
 * thrown error into a string result, so that failure is audited as completed with the error
 * text; only errors that escape `invoke` are audited as failed. The wrapper replaces `invoke`,
 * so the tool still parses its JSON input itself, and a dynamic `needsApproval` sees the model's
 * arguments before that parsing. Hosted and other non-function tools pass through unpublished
 * and unaudited.
 */
export function withTildeTools(
  ctx: AgentContext,
  tools: Tool[],
  options: BundledToolOptions = {},
): Promise<Tool[]> {
  return invocationTools(ctx, tools, options);
}

/** @internal `withTildeTools` with options for the channel's tools, as the invocation helper uses it. */
export async function invocationTools(
  ctx: AgentContext,
  tools: Tool[],
  options: BundledToolOptions,
  channel: ConvertToOpenAIAgentsToolsOptions = {},
): Promise<Tool[]> {
  const result = convertToOpenAIAgentsTools({ ...ctx.channel.current, ...ctx.agentTools }, channel);
  const names = new Set(result.map((t) => t.name));
  const bundled: BundledTool[] = [];
  for (const native of tools) {
    const described = describeTool(native, options);
    if (!described || native.type !== "function") {
      result.push(native);
      continue;
    }
    const name = native.name;
    if (!/^[a-zA-Z0-9_]{1,64}$/.test(name) || names.has(name))
      throw new Error(`Tool ${name} is not a valid model tool name or conflicts with another tool`);
    names.add(name);
    const invoke: FunctionTool["invoke"] = (runContext, input, details) =>
      auditToolCall(
        ctx,
        {
          name,
          toolCallId: details?.toolCall?.callId ?? randomUUID(),
          input: parseJson(input),
          summary: described.summary,
          display: described.display,
        },
        () => native.invoke(runContext, input, details),
      );
    result.push({ ...native, invoke } as FunctionTool);
    bundled.push({
      ...described,
      execute: (input, { toolCallId, frameworkContext }) => {
        const routed = frameworkContext as RoutedContext | undefined;
        const args = JSON.stringify(input);
        return invoke(routed?.runContext ?? new RunContext(), args, {
          toolCall: { type: "function_call", callId: toolCallId, name, arguments: args },
          signal: routed?.signal ?? ctx.signal,
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
 * Function tools carry no Tilde metadata, so summary, display and annotations are `options`'.
 * Undefined for hosted and other non-function tools.
 */
export function describeTool(
  native: Tool,
  options: BundledToolOptions,
): Omit<DiscoveredTool, "origin"> | undefined {
  if (native.type !== "function") return undefined;
  return {
    name: native.name,
    description: native.description,
    ...options[native.name],
    inputSchema: native.parameters as Record<string, unknown>,
    outputSchema: native.outputSchema as Record<string, unknown> | undefined,
  };
}

function parseJson(input: string): unknown {
  try {
    return JSON.parse(input);
  } catch {
    return input;
  }
}
