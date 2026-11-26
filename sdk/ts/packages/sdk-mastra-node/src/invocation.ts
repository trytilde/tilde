import { readdir } from "node:fs/promises";
import type { AgentContext, BundledTools } from "@trytilde/sdk";
import type { MastraDBMessage } from "@mastra/core/agent";
import type { InputProcessor } from "@mastra/core/processors";
import { RequestContext } from "@mastra/core/request-context";
import type { AgentSkillsResolver, SkillInput } from "@mastra/core/skills";
import { invocationTools, type AnyTool, type ConvertToMastraToolsOptions } from "./tools.js";
import { dynamicInstructionsStamp } from "./discover.js";
import { restore, toJson } from "./suspension.js";

/** Request context key holding the invocation's registry skills directory, resolved lazily. */
const SKILLS = "tilde.skills";

/**
 * The per-invocation pieces of a Mastra call, as `generate`/`stream` options:
 * `agent.generate(messages, { ...(await tildeMastra(ctx, { bundled })), maxSteps: 8 })`.
 *
 * - `toolsets.tilde`: the current channel's tools (see `convertToMastraTools`), the agent's
 *   other Tilde tools and its bundled tools (`options.bundled`, a `defineTools(...)`, audited
 *   and published as `withTildeTools` does).
 * - `abortSignal`: the invocation's signal, so a stop or suspension ends the loop.
 * - `inputProcessors`: before every step, steering input sent while the agent works is
 *   added as user messages, and dynamic instructions stamp this invocation's model calls. A
 *   Tilde tool call still running after about a minute, or Tilde's deadline, suspends the loop
 *   there (the message list is saved and the invocation ends); the run's next invocation
 *   continues that message list with the call's outcome (see `./suspension.ts`).
 *   Per-call processors replace the agent's own configured `inputProcessors` (Mastra's
 *   memory and skills processors stay); add yours to the array when the agent has some.
 * - `requestContext`: carries the skills assigned in Tilde's registry to an agent whose
 *   `skills` is `tildeMastraSkills(...)`. Pass your own as `options.requestContext`.
 */
export async function tildeMastra(
  ctx: AgentContext,
  options: ConvertToMastraToolsOptions & {
    requestContext?: RequestContext;
    bundled?: BundledTools<Record<string, AnyTool>>;
  } = {},
) {
  const { requestContext = new RequestContext(), bundled, ...channel } = options;
  let directory: Promise<string> | undefined;
  requestContext.set(SKILLS, () => (directory ??= ctx.skills.directory()));
  let restored = false;
  // The message list at the last step boundary. A loop that ends at `maxSteps` on a step whose
  // calls answered pending suspends with this, so that step runs again on resume: Mastra has no
  // per-call hook after a step's tool results short of replacing the agent's output processors.
  let latest: MastraDBMessage[] = [];
  ctx.enableSuspension?.(() => toJson(latest));
  const invocation: InputProcessor = {
    id: "tilde-invocation",
    async processInputStep({ messageList, agent, abort }) {
      const stamp = agent && dynamicInstructionsStamp(agent);
      if (stamp) ctx.activatePrompt(stamp);
      // A resumed run continues its saved conversation instead of the call's own messages.
      if (ctx.resumed && !restored) {
        restored = true;
        messageList.clear.all.db();
        messageList.add(restore(ctx.resumed), "memory");
      }
      for (const input of ctx.takeInputs())
        messageList.add({ role: "user", content: input.text }, "input");
      latest = messageList.get.all.db();
      if (ctx.shouldSuspend)
        await ctx.suspend(toJson(latest)).catch((error: unknown) => {
          // Saved: end the loop as a tripwire does, which Mastra treats as a normal end.
          if (ctx.signal.aborted) abort("Suspended until a Tilde tool call finishes");
          throw error;
        });
      return messageList;
    },
  };
  return {
    toolsets: {
      tilde: await invocationTools(ctx, bundled?.tools ?? {}, bundled?.options ?? {}, channel),
    },
    abortSignal: ctx.signal,
    inputProcessors: [invocation],
    requestContext,
  };
}

/**
 * The agent's `skills` plus those assigned to it in Tilde's registry, which reach every
 * deployment on its next invocation with no redeploy. Mastra has no per-call skills option,
 * so this is its dynamic `skills` form: `skills: tildeMastraSkills(["./skills"])`. The
 * registry skills arrive through `tildeMastra(ctx)`'s request context, written to a cache
 * directory (`ctx.skills.directory()`); outside an invocation (including `tilde deploy`,
 * which ships the bundled skills) only `skills` are listed.
 */
export function tildeMastraSkills(skills: SkillInput[] = []): AgentSkillsResolver {
  return async ({ requestContext }) => {
    const directory = requestContext.get(SKILLS) as (() => Promise<string>) | undefined;
    if (!directory) return skills;
    const root = await directory();
    return (await readdir(root)).length ? [...skills, root] : skills;
  };
}
