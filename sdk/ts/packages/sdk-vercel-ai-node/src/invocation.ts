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
  onStepFinish?: (step: { response: { messages: ModelMessage[] } }) => unknown;
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
 * - Resumed (`ctx.takeResumed()`, the invocation's first loop): the saved conversation replaces the call's own messages, with each
 *   pending tool result replaced by the call's outcome (see `./suspension.ts`).
 * - Steering sent while the agent works becomes user messages where it arrived.
 * - Suspension: a Tilde tool call that answered pending, or Tilde's deadline, saves the
 *   conversation so far (`ctx.suspend`) and ends the invocation; the run resumes with the
 *   outcome in a new invocation.
 */
type Loop = {
  prepareStep: PrepareStepFunction<any>;
  onStepFinish: (step: { response: { messages: ModelMessage[] } }) => void;
  /** The conversation at the last step boundary or step end, as the model saw it. */
  latest: ModelMessage[];
};
/** Each invocation's current loop, which `tildeAiSdk`'s `onStepFinish` and `conversation` reach. */
const loops = new WeakMap<AgentContext, Loop>();

function invocationStep(ctx: AgentContext, inner?: PrepareStepFunction<any>): Loop {
  const steering: { at: number; message: ModelMessage }[] = [];
  let first: ModelMessage[] | undefined;
  let restored: ModelMessage[] | undefined;
  // The conversation for the responses so far (cumulative in the AI SDK): the call's messages
  // (or the saved ones), the responses and steering where it arrived.
  const assemble = (responses: ModelMessage[]) => {
    const own = [...(restored ?? first ?? []), ...responses];
    steering.forEach((input, i) => own.splice(input.at + i, 0, input.message));
    return own;
  };
  const loop: Loop = {
    latest: [],
    prepareStep: async (step) => {
      if (first === undefined) {
        first = step.messages;
        const resumed = ctx.takeResumed?.();
        if (resumed) restored = restore(resumed);
      }
      const responses = step.messages.slice(first.length);
      for (const input of ctx.takeInputs())
        steering.push({
          at: (restored ?? first).length + responses.length,
          message: { role: "user", content: input.text },
        });
      const messages = assemble(responses);
      loop.latest = messages;
      if (ctx.shouldSuspend) await ctx.suspend(toJson(messages));
      if (!restored && !steering.length) return inner?.(step);
      const result = await inner?.({ ...step, messages });
      return { ...result, messages: result?.messages ?? messages };
    },
    // A loop that ends on a step whose calls answered pending suspends with that step, so
    // resuming does not call them again. Only a snapshot: steering waits for the next step.
    onStepFinish: (step) => {
      loop.latest = assemble(step.response.messages);
    },
  };
  loops.set(ctx, loop);
  ctx.enableSuspension?.(() => toJson(loop.latest));
  return loop;
}

/**
 * The conversation the invocation's last `prepareTildeCall` loop ended with, as its model saw
 * it: on a resumed invocation that is the saved conversation and what followed, which the
 * call's own messages do not hold. Continue from it for a second loop in the same invocation.
 */
export function tildeConversation(ctx: AgentContext): ModelMessage[] | undefined {
  return loops.get(ctx)?.latest;
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
  const step = invocationStep(ctx, own.prepareStep);
  const key = "system" in own && !("instructions" in own) ? "system" : "instructions";
  return {
    ...own,
    tools: {
      ...own.tools,
      ...(await invocationTools(ctx, bundled?.tools ?? {}, bundled?.options ?? {}, channel)),
      ...(summary ? convertToAiSdkTools(ctx.skills.tools()) : {}),
    },
    [key]: withSummary(own[key], summary),
    prepareStep: step.prepareStep,
    // generateText/streamText; a ToolLoopAgent's call takes it from `tildeAiSdk`.
    onStepFinish: async (finished: Parameters<NonNullable<Settings["onStepFinish"]>>[0]) => {
      await own.onStepFinish?.(finished);
      step.onStepFinish(finished);
    },
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
    // `prepareCall` cannot set it: records each step for suspension (see `prepareTildeCall`).
    onStepFinish: (step: { response: { messages: ModelMessage[] } }) =>
      loops.get(ctx)?.onStepFinish(step),
  };
}
