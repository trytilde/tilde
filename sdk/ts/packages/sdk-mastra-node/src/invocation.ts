import { readdir } from "node:fs/promises";
import type { AgentContext, BundledTools } from "@trytilde/sdk";
import type { InputProcessor } from "@mastra/core/processors";
import { RequestContext } from "@mastra/core/request-context";
import type { AgentSkillsResolver, SkillInput } from "@mastra/core/skills";
import { invocationTools, type AnyTool, type ConvertToMastraToolsOptions } from "./tools.js";
import { dynamicInstructionsStamp } from "./discover.js";

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
 *   added as user messages, and dynamic instructions stamp this invocation's model calls.
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
  const invocation: InputProcessor = {
    id: "tilde-invocation",
    processInputStep({ messageList, agent }) {
      const stamp = agent && dynamicInstructionsStamp(agent);
      if (stamp) ctx.activatePrompt(stamp);
      for (const input of ctx.takeInputs())
        messageList.add({ role: "user", content: input.text }, "input");
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
