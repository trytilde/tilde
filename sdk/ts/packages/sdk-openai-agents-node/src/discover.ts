import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import {
  PromptFormat,
  canonicalJson,
  isBundledTools,
  promptHash,
  type BundledToolOptions,
  type DiscoveredPrompt,
  type DiscoveredTool,
  type Discovery,
  type DiscoveryContext,
} from "@trytilde/sdk";
import type { Agent, Tool } from "@openai/agents";
import { describeTool } from "./tools.js";

type AnyAgent = Agent<any, any>;
// Duck-typed rather than `instanceof`: the project's @openai/agents may be another copy.
export const isAgent = (value: unknown): value is AnyAgent =>
  !!value &&
  typeof value === "object" &&
  typeof (value as AnyAgent).name === "string" &&
  "instructions" in value &&
  Array.isArray((value as AnyAgent).handoffs) &&
  Array.isArray((value as AnyAgent).tools) &&
  typeof (value as AnyAgent).clone === "function" &&
  typeof (value as AnyAgent).asTool === "function";

// OpenAI Agents tools are tagged objects (`function`, `hosted_tool`, `shell`, ...).
const isTool = (value: unknown): value is Tool =>
  !!value &&
  typeof value === "object" &&
  typeof (value as Tool).type === "string" &&
  typeof (value as Tool).name === "string";
/** Tools as `withTildeTools` publishes them, each at `<origin>.<name>`. */
function discoverTools(tools: Tool[], options: BundledToolOptions, origin: string) {
  const found: DiscoveredTool[] = [];
  for (const tool of tools) {
    const described = describeTool(tool, options);
    if (described) found.push({ ...described, origin: `${origin}.${tool.name}` });
  }
  return found;
}

/** Prompt names allow `[A-Za-z0-9._/-]`; agent names are free text. */
const promptName = (agent: AnyAgent, field: string) =>
  `${agent.name.replace(/[^A-Za-z0-9._-]/g, "-")}/${field}`;

/** `instructions` as a prompt: a string is plain, a function dynamic with its source. */
export function instructionsPrompt(agent: AnyAgent, origin = ""): DiscoveredPrompt | undefined {
  const { instructions } = agent;
  if (typeof instructions === "function")
    return {
      name: promptName(agent, "instructions"),
      format: PromptFormat.DYNAMIC,
      template: instructions.toString(),
      origin: `${origin}.instructions`,
    };
  if (!instructions) return undefined;
  return {
    name: promptName(agent, "instructions"),
    format: PromptFormat.PLAIN,
    template: instructions,
    origin: `${origin}.instructions`,
  };
}
/** Stamp of the agent's dynamic instructions, or undefined when they are static. */
export function dynamicInstructionsStamp(agent: AnyAgent) {
  const prompt = instructionsPrompt(agent);
  if (prompt?.format !== PromptFormat.DYNAMIC) return undefined;
  return { name: prompt.name, hash: promptHash(prompt.template, {}, canonicalJson({})) };
}

let toolSource: Promise<((tool: Tool) => AnyAgent | undefined) | undefined> | undefined;
/**
 * The agent behind an `agent.asTool()` tool. @openai/agents 0.18 keeps that link only in an
 * internal registry (`agentToolSourceRegistry`) outside its package exports, so it is loaded by
 * file from the same agents-core copy `@openai/agents` uses. Undefined when that is not possible.
 */
function agentToolSource() {
  toolSource ??= (async () => {
    try {
      const agents = createRequire(import.meta.url).resolve("@openai/agents");
      const core = dirname(createRequire(agents).resolve("@openai/agents-core"));
      const registry = await import(pathToFileURL(join(core, "agentToolSourceRegistry.mjs")).href);
      return typeof registry.getAgentToolSourceAgent === "function"
        ? (registry.getAgentToolSourceAgent as (tool: Tool) => AnyAgent | undefined)
        : undefined;
    } catch {
      return undefined;
    }
  })();
  return toolSource;
}
const isAgentTool = (tool: Tool) =>
  tool.type === "function" && typeof (tool as { on?: unknown }).on === "function";

/**
 * Every agent reachable from `root` through handoffs and agent tools, root first, each once
 * (handoffs may form cycles), with its origin.
 */
export async function agentGraph(root: AnyAgent, origin = "") {
  const source = await agentToolSource();
  const found = new Map<AnyAgent, string>();
  const warnings: string[] = [];
  const visit = (agent: AnyAgent, path: string) => {
    if (found.has(agent)) return;
    found.set(agent, path);
    agent.handoffs.forEach((handoff, i) =>
      visit(isAgent(handoff) ? handoff : handoff.agent, `${path}.handoffs[${i}]`),
    );
    agent.tools.forEach((tool, i) => {
      if (!isAgentTool(tool)) return;
      const nested = source?.(tool);
      if (nested) visit(nested, `${path}.tools[${i}]`);
      else
        warnings.push(
          `OpenAI agent ${agent.name}: the agent behind tool ${tool.name} could not be read; export it to version its prompts (${path})`,
        );
    });
  };
  visit(root, origin);
  return { agents: [...found], warnings };
}

type SkillsCapability = {
  type: "skills";
  from?: { type: string; src?: string };
  lazyFrom?: { source: { type: string; src?: string } };
  skills?: unknown[];
};
function discoverAgent(agent: AnyAgent, origin: string, discovery: Required<Discovery>) {
  const prompt = instructionsPrompt(agent, origin);
  if (prompt) discovery.prompts.push(prompt);
  if (agent.handoffDescription)
    discovery.prompts.push({
      name: promptName(agent, "handoff_description"),
      format: PromptFormat.PLAIN,
      template: agent.handoffDescription,
      origin: `${origin}.handoffDescription`,
    });
  if (agent.prompt)
    discovery.warnings.push(
      `OpenAI agent ${agent.name}: its hosted prompt (prompt: { id }) is versioned by OpenAI, not Tilde (${origin}.prompt)`,
    );
  // Shell tool local skills: `shellTool({ environment: { type: "local", skills: [...] } })`.
  agent.tools.forEach((tool, i) => {
    if (tool.type !== "shell" || tool.environment?.type !== "local") return;
    for (const skill of tool.environment.skills ?? [])
      discovery.skills.push({
        dir: resolve(process.cwd(), skill.path),
        origin: `${origin}.tools[${i}].environment.skills`,
      });
  });
  // SandboxAgent `skills({ from: localDir(...) })` capabilities; relative paths resolve from the cwd.
  const capabilities = (agent as { capabilities?: unknown }).capabilities;
  if (!Array.isArray(capabilities)) return;
  capabilities.forEach((capability: SkillsCapability, i) => {
    if (capability?.type !== "skills") return;
    const entry = capability.from ?? capability.lazyFrom?.source;
    if (entry?.type === "local_dir" && entry.src)
      discovery.skills.push({
        dir: resolve(process.cwd(), entry.src),
        origin: `${origin}.capabilities[${i}]`,
      });
    else if (entry || capability.skills?.length)
      discovery.warnings.push(
        `OpenAI agent ${agent.name}: only local_dir sandbox skills are read (${origin}.capabilities[${i}])`,
      );
  });
}

/**
 * `tilde deploy` calls this for each export of the entry module. An `Agent` contributes its
 * instructions (`<agent name>/instructions`), handoff description and local skills, and those
 * of every agent reachable through its handoffs and agent tools, plus its own function tools;
 * a `defineTools` value of OpenAI Agents tools its function tools.
 */
export async function discover(
  value: unknown,
  context: DiscoveryContext,
): Promise<Discovery | undefined> {
  if (isBundledTools(value) && Array.isArray(value.tools) && value.tools.every(isTool))
    return { tools: discoverTools(value.tools, value.options, `${context.origin}.tools`) };
  if (!isAgent(value)) return undefined;
  const { agents, warnings } = await agentGraph(value, context.origin);
  // Handoff and nested agents' tools are theirs, not this agent's, so only its own are declared.
  const tools = discoverTools(value.tools, {}, `${context.origin}.tools`);
  const discovery: Required<Discovery> = { prompts: [], skills: [], tools, warnings };
  for (const [agent, origin] of agents) discoverAgent(agent, origin, discovery);
  return discovery;
}
