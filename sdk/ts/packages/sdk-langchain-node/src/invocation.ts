import type { AgentContext, BundledTools } from "@trytilde/sdk";
import { HumanMessage } from "@langchain/core/messages";
import type { StructuredToolInterface } from "@langchain/core/tools";
import { createMiddleware } from "langchain";
import {
  convertToLangChainTools,
  invocationTools as tildeTools,
  type ConvertToLangChainToolsOptions,
} from "./tools.js";

/** Options for the channel's tools, plus the agent's bundled tools (`defineTools`). */
export type TildeToolsOptions = ConvertToLangChainToolsOptions & {
  bundled?: BundledTools<StructuredToolInterface[]>;
};

/**
 * Where `tildeLangChain(ctx)` puts the invocation: `configurable`, which every middleware hook
 * receives (`runtime.context` reaches before/after hooks only through a declared schema). The
 * `__` prefix keeps it out of tracing metadata, as LangGraph's own `__pregel_*` objects.
 */
const invocation = "__tilde_invocation";

function current(runtime: { configurable?: Record<string, unknown> }): AgentContext {
  const ctx = runtime.configurable?.[invocation] as AgentContext | undefined;
  if (!ctx)
    throw new Error(
      "tildeMiddleware() needs agent.invoke(input, { ...tildeLangChain(ctx) }) inside run(ctx)",
    );
  return ctx;
}

/**
 * Tilde's per-invocation pieces as LangChain middleware, added once where the agent is created:
 * `createAgent({ model, systemPrompt, middleware: [tildeMiddleware({ bundled })] })`, where
 * `bundled` is the agent's `defineTools(...)`. `createAgent` fixes
 * its middleware and tools when it builds the graph, so the middleware reads the running
 * invocation from the call's runtime context (`tildeLangChain(ctx)`) and, per model call:
 *
 * - adds the current channel's tools (see `convertToLangChainTools`), the agent's other Tilde
 *   tools and its bundled tools (audited and published as `withTildeTools` does), and runs them
 *   when the model calls them (LangChain executes tools added in `wrapModelCall` through
 *   `wrapToolCall`);
 * - before each model call, adds steering input sent while the agent works as user messages
 *   to the agent state;
 * - offers the invocation's skills (LangChain has no native skills): Tilde's `list_skills` and
 *   `read_skill` tools, with the skill summary appended to the system message.
 */
export function tildeMiddleware(tools: TildeToolsOptions = {}) {
  const { bundled, ...channel } = tools;
  const added = new WeakMap<AgentContext, Promise<StructuredToolInterface[]>>();
  const summaries = new WeakMap<AgentContext, Promise<string>>();
  const summary = (ctx: AgentContext) => {
    if (!summaries.has(ctx)) summaries.set(ctx, ctx.skills.summary());
    return summaries.get(ctx)!;
  };
  // Built once per invocation; the channel's tools do not change while it runs.
  const invocationTools = (ctx: AgentContext) => {
    if (!added.has(ctx))
      added.set(
        ctx,
        summary(ctx).then(async (text) => [
          ...(await tildeTools(ctx, bundled?.tools ?? [], bundled?.options ?? {}, channel)),
          ...(text ? convertToLangChainTools(ctx.skills.tools()) : []),
        ]),
      );
    return added.get(ctx)!;
  };
  return createMiddleware({
    name: "tilde",
    beforeModel: (_state, runtime) => {
      const inputs = current(runtime).takeInputs();
      return inputs.length
        ? { messages: inputs.map((input) => new HumanMessage(input.text)) }
        : undefined;
    },
    wrapModelCall: async (request, handler) => {
      const ctx = current(request.runtime);
      const text = await summary(ctx);
      return handler({
        ...request,
        tools: [...request.tools, ...(await invocationTools(ctx))],
        systemMessage: text ? request.systemMessage.concat(`\n\n${text}`) : request.systemMessage,
      });
    },
    wrapToolCall: async (request, handler) => {
      if (request.tool) return handler(request);
      const own = (await invocationTools(current(request.runtime))).find(
        (tool) => tool.name === request.toolCall.name,
      );
      return handler(own ? { ...request, tool: own } : request);
    },
  });
}

/**
 * The per-invocation options of a LangChain agent call, for an agent created with
 * `tildeMiddleware()`: `agent.invoke({ messages }, { ...tildeLangChain(ctx), recursionLimit: 16 })`.
 * `signal` ends the run on a stop or suspension; `configurable` carries the invocation to the
 * middleware (spread it when you pass your own, e.g. a `thread_id`).
 */
export function tildeLangChain(ctx: AgentContext) {
  return { signal: ctx.signal, configurable: { [invocation]: ctx } };
}
