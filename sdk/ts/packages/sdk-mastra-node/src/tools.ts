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
import { standardSchemaToJSONSchema } from "@mastra/core/schema";
import { createTool, isValidationError, Tool } from "@mastra/core/tools";
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
    const key = modelToolName(`${options.prefix ?? ""}${name}`);
    if (!key || Object.hasOwn(result, key))
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
      // A routed tools.execute hands this context to the bundled tool it names.
      execute: async (input, context) => {
        context.abortSignal?.throwIfAborted();
        return channel.execute(input, {
          toolCallId: context.agent?.toolCallId || randomUUID(),
          frameworkContext: context,
        });
      },
    });
  }
  return result;
}

// Tools of any schema, as Mastra's Agent accepts them.
export type AnyTool = Tool<any, any, any, any, any, any, any>;
type ExecutionContext = {
  toolCallId?: string;
  agent?: { toolCallId?: string };
  abortSignal?: AbortSignal;
  requestContext?: unknown;
};

/**
 * The agent's tools for a Mastra `Agent` in one record: the current channel's tools, its other
 * Tilde tools (`ctx.agentTools`) and `tools`, its own `createTool` tools, keyed by the name the
 * model sees. Those are published to Tilde for this invocation, so `tools.search`/`tools.schemas`
 * describe them and a `tools.execute` naming one runs it here; every call is audited once, with
 * the model's call ID. Annotations come from a tool's `mcp.annotations` (with MCP's defaults);
 * summary and display from `mcp._meta.tilde`; `options[name]` overrides both.
 *
 * Caveats: Mastra returns a `ValidationError` for invalid input or output instead of throwing;
 * it is audited as failed and still returned to Mastra. Entries that are not Mastra tools
 * (AI SDK tools in the record) or have no `execute` pass through unpublished and unaudited.
 * A `tools.execute` runs the tool with the outer call's `requestContext` but outside the agent
 * loop, so suspend/resume and the agent's message list are unavailable to it.
 */
export function withTildeTools(
  ctx: AgentContext,
  tools: Record<string, AnyTool>,
  options: BundledToolOptions = {},
): Promise<Record<string, AnyTool>> {
  return invocationTools(ctx, tools, options);
}

/** @internal `withTildeTools` with options for the channel's tools, as `tildeMastra` uses it. */
export async function invocationTools(
  ctx: AgentContext,
  tools: Record<string, AnyTool>,
  options: BundledToolOptions,
  channel: ConvertToMastraToolsOptions = {},
): Promise<Record<string, AnyTool>> {
  const result: Record<string, AnyTool> = convertToMastraTools(
    { ...ctx.channel.current, ...ctx.agentTools },
    channel,
  );
  const bundled: BundledTool[] = [];
  for (const [name, native] of Object.entries(tools)) {
    if (!/^[a-zA-Z0-9_-]{1,64}$/.test(name) || Object.hasOwn(result, name))
      throw new Error(`Tool ${name} is not a valid model tool name or conflicts with another tool`);
    const described = describeTool(name, native, options);
    const execute = native.execute?.bind(native);
    if (!described || !execute) {
      result[name] = native;
      continue;
    }
    const run = (input: unknown, context?: ExecutionContext) =>
      auditToolCall(
        ctx,
        {
          name,
          // AI SDK-shaped contexts carry the ID flat; Mastra's agent loop nests it under agent.
          toolCallId: context?.toolCallId || context?.agent?.toolCallId || randomUUID(),
          input,
          summary: described.summary,
          display: described.display,
        },
        () => execute(input, context as never),
        (output) => (isValidationError(output) ? { error: output.message } : { output }),
      );
    result[name] = Object.assign(Object.create(Object.getPrototypeOf(native)), native, {
      execute: run,
    });
    bundled.push({
      ...described,
      execute: (input, { toolCallId, frameworkContext }) =>
        run(input, {
          toolCallId,
          messages: [],
          abortSignal: ctx.signal,
          requestContext: (frameworkContext as ExecutionContext | undefined)?.requestContext,
        } as ExecutionContext),
    });
  }
  // Published even when empty, so tools removed since the last call stop running here.
  await ctx.setBundledTools(bundled);
  return result;
}

/**
 * @internal How Tilde describes a native tool, for `withTildeTools` and `tilde deploy` alike.
 * Undefined for entries that are not Mastra tools or have no `execute`.
 */
export function describeTool(
  name: string,
  native: AnyTool,
  options: BundledToolOptions,
): Omit<DiscoveredTool, "origin"> | undefined {
  if (!(native instanceof Tool) || !native.execute) return undefined;
  const meta = native.mcp?._meta?.tilde as BundledToolOptions[string] | undefined;
  const hints = native.mcp?.annotations;
  return {
    name,
    description: native.description,
    ...meta,
    annotations: hints
      ? {
          readOnly: hints.readOnlyHint ?? false,
          destructive: hints.destructiveHint ?? true,
          idempotent: hints.idempotentHint ?? false,
          openWorld: hints.openWorldHint ?? true,
        }
      : meta?.annotations,
    ...options[name],
    inputSchema: native.inputSchema
      ? (standardSchemaToJSONSchema(native.inputSchema, { io: "input" }) as Record<string, unknown>)
      : { type: "object" },
    outputSchema: native.outputSchema
      ? (standardSchemaToJSONSchema(native.outputSchema, { io: "output" }) as Record<
          string,
          unknown
        >)
      : undefined,
  };
}
