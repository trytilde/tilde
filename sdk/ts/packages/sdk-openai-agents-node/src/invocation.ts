import { join } from "node:path";
import type { AgentContext, BundledTools } from "@trytilde/sdk";
import type { Agent, AgentInputItem, CallModelInputFilter, ShellTool, Tool } from "@openai/agents";
import { agentGraph, dynamicInstructionsStamp, isAgent } from "./discover.js";
import {
  convertToOpenAIAgentsTools,
  invocationTools,
  type ConvertToOpenAIAgentsToolsOptions,
} from "./tools.js";
import { restore, toJson } from "./suspension.js";

/** Options for the channel's tools, plus the agent's bundled tools (`defineTools`). */
export type TildeToolsOptions = ConvertToOpenAIAgentsToolsOptions & {
  bundled?: BundledTools<Tool[]>;
};

type AnyAgent = Agent<any, any>;
type Instructions = AnyAgent["instructions"];
const isLocalShell = (tool: Tool): tool is Extract<ShellTool, { shell: unknown }> =>
  tool.type === "shell" && !!tool.shell;
const hasLocalShell = (agent: AnyAgent) => agent.tools.some(isLocalShell);

/**
 * Registry skills (assigned in Tilde, not shipped with the deployment) as shell tool local
 * skills, in the folders `ctx.skills.directory()` writes them to.
 */
async function registrySkills(ctx: AgentContext) {
  const skills = (await ctx.skills.list()).filter((skill) => !skill.deployed);
  if (!skills.length) return [];
  const root = await ctx.skills.directory();
  const repeated = new Set(skills.map((s) => s.name).filter((n, i, all) => all.indexOf(n) !== i));
  return skills.map((skill) => ({
    name: skill.name,
    description: skill.description,
    path: join(root, repeated.has(skill.name) ? `${skill.source}-${skill.name}` : skill.name),
  }));
}

/**
 * The per-invocation pieces of an OpenAI Agents run: a copy of `agent` to run, and `run` options.
 *
 * ```ts
 * const tilde = await tildeOpenAIAgents(ctx, agent, { bundled });
 * await run(tilde.agent, items, { ...tilde.options, maxTurns: 8 });
 * ```
 *
 * - `agent`: a clone of `agent` and of every agent it hands off to, each with the current
 *   channel's tools (see `convertToOpenAIAgentsTools`), the agent's other Tilde tools, its
 *   bundled tools (`tools.bundled`, a `defineTools(...)`, audited and published as
 *   `withTildeTools` does) and this invocation's registry skills.
 *   Agents with a local shell tool get them as its local skills; the others get Tilde's
 *   `list_skills`/`read_skill` tools with the skill summary appended to their instructions.
 * - `options.signal`: the invocation's signal, so a stop or suspension ends the run.
 * - `options.callModelInputFilter`: before every model call, steering input sent while the agent
 *   works is added as user messages (kept at that point for later turns), and the running
 *   agent's dynamic instructions stamp this invocation's model calls. A Tilde tool call still
 *   running after about a minute, or Tilde's deadline, suspends the run there (its items are
 *   saved and the invocation ends); the run's next invocation continues from those items with the
 *   call's outcome in place of its pending answer.
 *
 * Agents used as tools (`agent.asTool()`) run nested runs with their own options: they keep
 * their tools and receive no steering.
 */
export async function tildeOpenAIAgents<TAgent extends AnyAgent>(
  ctx: AgentContext,
  agent: TAgent,
  tools: TildeToolsOptions = {},
): Promise<{
  agent: TAgent;
  options: { signal: AbortSignal; callModelInputFilter: CallModelInputFilter };
}> {
  const { agents } = await agentGraph(agent);
  const stamps = new Map(
    agents.flatMap(([a]) => {
      const stamp = dynamicInstructionsStamp(a);
      return stamp ? [[a.name, stamp] as const] : [];
    }),
  );
  const { bundled, ...options } = tools;
  const channel = await invocationTools(ctx, bundled?.tools ?? [], bundled?.options ?? {}, options);
  const localSkills = agents.some(([a]) => hasLocalShell(a)) ? await registrySkills(ctx) : [];
  const summary = agents.some(([a]) => !hasLocalShell(a)) ? await ctx.skills.summary() : "";
  const skillTools = summary ? convertToOpenAIAgentsTools(ctx.skills.tools()) : [];
  const instructions = (original: Instructions): Instructions =>
    !summary
      ? original
      : typeof original === "function"
        ? async (runContext, self) => `${await original(runContext, self)}\n\n${summary}`
        : original
          ? `${original}\n\n${summary}`
          : summary;

  const clones = new Map<AnyAgent, AnyAgent>();
  const copy = (original: AnyAgent): AnyAgent => {
    const existing = clones.get(original);
    if (existing) return existing;
    const own = original.tools.map((tool) =>
      isLocalShell(tool) && localSkills.length
        ? {
            ...tool,
            environment: {
              type: "local" as const,
              skills: [...(tool.environment?.skills ?? []), ...localSkills],
            },
          }
        : tool,
    );
    const native = hasLocalShell(original);
    const clone = original.clone({
      tools: [...own, ...channel, ...(native ? [] : skillTools)],
      instructions: native ? original.instructions : instructions(original.instructions),
    });
    clones.set(original, clone);
    // Assigned after registering the clone: handoffs may lead back to this agent.
    clone.handoffs = original.handoffs.map((handoff) =>
      isAgent(handoff) ? copy(handoff) : handoff.clone({ agent: copy(handoff.agent) }),
    );
    return clone;
  };

  const steered: AgentInputItem[] = [];
  const positions: number[] = [];
  let initial: number | undefined;
  let restored: AgentInputItem[] | undefined;
  let latest: AgentInputItem[] = [];
  ctx.enableSuspension?.(() => toJson(latest));
  const callModelInputFilter: CallModelInputFilter = async ({ modelData, agent: running }) => {
    const stamp = stamps.get(running.name);
    if (stamp) ctx.activatePrompt(stamp);
    // A resumed run continues its saved items instead of the run's own input.
    if (initial === undefined) {
      initial = modelData.input.length;
      if (ctx.resumed) restored = restore(ctx.resumed);
    }
    const own = restored ? [...restored, ...modelData.input.slice(initial)] : modelData.input;
    // The runner rebuilds the input each turn without filter additions, so earlier steering is
    // put back where it arrived; the input only grows at its end between turns.
    for (const input of ctx.takeInputs()) {
      positions.push(own.length + steered.length);
      steered.push({ role: "user", content: input.text });
    }
    const input = [...own];
    steered.forEach((item, i) => input.splice(Math.min(positions[i], input.length), 0, item));
    latest = input;
    if (ctx.shouldSuspend) await ctx.suspend(toJson(input));
    return { ...modelData, input };
  };
  return {
    agent: copy(agent) as TAgent,
    options: { signal: ctx.signal, callModelInputFilter },
  };
}
