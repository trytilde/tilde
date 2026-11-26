import type { AgentContext, BundledTools } from "@trytilde/sdk";
import {
  jsonSchema,
  type ModelMessage,
  type PrepareStepFunction,
  type SystemModelMessage,
  type ToolSet,
} from "ai";
import { convertToAiSdkTools, invocationTools, type ConvertToAiSdkToolsOptions } from "./tools.js";
import { restore, toJson } from "./suspension.js";

/** Options for the channel's tools, plus the agent's bundled tools (`defineTools`). */
export type TildeToolsOptions = ConvertToAiSdkToolsOptions & { bundled?: BundledTools<ToolSet> };
export type TildeCallOptions = {
  tilde: { context: AgentContext; tools?: TildeToolsOptions };
};
type Instructions = string | SystemModelMessage | SystemModelMessage[];
type Settings = {
  tools?: ToolSet;
  instructions?: Instructions;
  system?: Instructions;
  prepareStep?: PrepareStepFunction<any>;
};

function withSummary(instructions: Instructions | undefined, summary: string) {
  if (!summary) return instructions;
  if (instructions === undefined || instructions === "") return summary;
  if (typeof instructions === "string") return `${instructions}\n\n${summary}`;
  // Message instructions may carry provider options: add the summary as its own message.
  return [
    ...(Array.isArray(instructions) ? instructions : [instructions]),
    { role: "system", content: summary },
  ];
}

/**
 * Before every step: continue a suspended run, hand steering input to the model and suspend when
 * Tilde asks. The AI SDK rebuilds each step's messages from the call's messages and the responses
 * so far, so every step re-applies what earlier ones changed:
 *
 * - Resumed (`ctx.resumed`): the saved conversation replaces the call's own messages, with each
 *   pending tool result replaced by the call's outcome (see `./suspension.ts`).
 * - Steering sent while the agent works becomes user messages where it arrived.
 * - Suspension: a Tilde tool call that answered pending, or Tilde's deadline, saves the
 *   conversation so far (`ctx.suspend`) and ends the invocation; the run resumes with the
 *   outcome in a new invocation.
 */
function invocationStep(
  ctx: AgentContext,
  inner?: PrepareStepFunction<any>,
): PrepareStepFunction<any> {
  const steering: { at: number; message: ModelMessage }[] = [];
  let initial: number | undefined;
  let restored: ModelMessage[] | undefined;
  let latest: ModelMessage[] = [];
  ctx.enableSuspension?.(() => toJson(latest));
  return async (step) => {
    if (initial === undefined) {
      initial = step.messages.length;
      if (ctx.resumed) restored = restore(ctx.resumed);
    }
    const own = restored ? [...restored, ...step.messages.slice(initial)] : step.messages;
    for (const input of ctx.takeInputs())
      steering.push({ at: own.length, message: { role: "user", content: input.text } });
    const messages = [...own];
    steering.forEach((input, i) => messages.splice(input.at + i, 0, input.message));
    latest = messages;
    if (ctx.shouldSuspend) await ctx.suspend(toJson(messages));
    if (!restored && !steering.length) return inner?.(step);
    const result = await inner?.({ ...step, messages });
    return { ...result, messages: result?.messages ?? messages };
  };
}

/**
 * Add Tilde's per-invocation pieces to AI SDK call settings: the current channel's tools, the
 * agent's other Tilde tools (`ctx.agentTools`), its bundled tools (`tools.bundled`, audited and
 * published as `withTildeTools` does), the agent's Tilde skills (the AI SDK has no native skills: `list_skills`/`read_skill` tools and
 * `ctx.skills.summary()` appended to `instructions`, or `system` for `generateText`), and a
 * `prepareStep` (wrapping the settings' own) that hands steering input to the model at every
 * step, suspends the loop when a Tilde tool call is still running after about a minute or Tilde's
 * deadline comes, and continues a resumed run from its saved conversation. ToolLoopAgent instructions are static text the gateway matches, so nothing is stamped.
 *
 * `tildeCallOptions` calls this from a `ToolLoopAgent`'s `prepareCall`; call it yourself for
 * `generateText`/`streamText`:
 * `generateText({ ...(await prepareTildeCall(ctx, { model, system, messages })), abortSignal: ctx.signal })`.
 */
export async function prepareTildeCall<S extends object>(
  ctx: AgentContext,
  settings: S,
  tools: TildeToolsOptions = {},
): Promise<S> {
  const own = settings as S & Settings;
  const { bundled, ...channel } = tools;
  const summary = await ctx.skills.summary();
  const key = "system" in own && !("instructions" in own) ? "system" : "instructions";
  return {
    ...own,
    tools: {
      ...own.tools,
      ...(await invocationTools(ctx, bundled?.tools ?? {}, bundled?.options ?? {}, channel)),
      ...(summary ? convertToAiSdkTools(ctx.skills.tools()) : {}),
    },
    [key]: withSummary(own[key], summary),
    prepareStep: invocationStep(ctx, own.prepareStep),
  };
}

/**
 * Spread into a `ToolLoopAgent`'s settings so `agent.generate/stream` take Tilde's
 * per-invocation pieces as call options (the AI SDK's `callOptionsSchema` + `prepareCall`;
 * `generate`/`stream` accept no per-call tools or instructions otherwise):
 * `new ToolLoopAgent({ model, instructions, ...tildeCallOptions })`, then
 * `agent.generate({ messages, ...tildeAiSdk(ctx) })`.
 *
 * An agent with call options of its own adds `tilde` to its schema and ends its `prepareCall`
 * with `prepareTildeCall(options.tilde.context, settings, options.tilde.tools)`.
 */
export const tildeCallOptions = {
  // Passes the value through unchanged: an AgentContext is not JSON.
  callOptionsSchema: jsonSchema<TildeCallOptions>({ type: "object" }),
  prepareCall: <S extends { options: TildeCallOptions }>({ options, ...settings }: S) =>
    prepareTildeCall(options.tilde.context, settings, options.tilde.tools),
};

/**
 * The per-invocation call options for an agent built with `tildeCallOptions`:
 * `agent.generate({ messages, ...tildeAiSdk(ctx, { bundled }) })`, where `bundled` is the
 * agent's `defineTools(...)`. Carries the invocation to `prepareCall` and its `abortSignal`, so
 * a stop or suspension ends the loop.
 */
export function tildeAiSdk(ctx: AgentContext, tools?: TildeToolsOptions) {
  return {
    options: { tilde: { context: ctx, tools } } satisfies TildeCallOptions,
    abortSignal: ctx.signal,
  };
}
