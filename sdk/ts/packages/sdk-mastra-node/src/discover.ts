import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import {
  PromptFormat,
  canonicalJson,
  isBundledTools,
  promptHash,
  type BundledToolOptions,
  type DiscoveredPrompt,
  type DiscoveredSkill,
  type DiscoveredTool,
  type Discovery,
  type DiscoveryContext,
} from "@trytilde/sdk";
import type { Agent } from "@mastra/core/agent";
import type { Mastra } from "@mastra/core";
import { Tool } from "@mastra/core/tools";
import { describeTool, type AnyTool } from "./tools.js";

// Duck-typed rather than `instanceof`: the project's @mastra/core may be another copy.
const isAgent = (value: unknown): value is Agent =>
  !!value &&
  typeof value === "object" &&
  typeof (value as Agent).id === "string" &&
  typeof (value as Agent).getInstructions === "function" &&
  typeof (value as Agent).listSkills === "function";
const isMastra = (value: unknown): value is Mastra =>
  !!value &&
  typeof value === "object" &&
  typeof (value as Mastra).listAgents === "function" &&
  typeof (value as Mastra).getAgent === "function";

/** Tools as `withTildeTools` publishes them, each at `<origin>.<name>`. */
function discoverTools(
  tools: Record<string, AnyTool>,
  options: BundledToolOptions,
  origin: string,
): DiscoveredTool[] {
  const found: DiscoveredTool[] = [];
  for (const [name, native] of Object.entries(tools)) {
    const described = describeTool(name, native, options);
    if (described) found.push({ ...described, origin: `${origin}.${name}` });
  }
  return found;
}

type Instructions = string | { content?: unknown } | (string | { content?: unknown })[];
/**
 * The instructions and tools exactly as the agent was constructed with them, without calling a
 * function. Mastra has no public getter for that: `getInstructions()` and `listTools()` run
 * functions, so this reads `__getOverridableFields()` (Mastra's own editor snapshot, @mastra/core 1.x).
 */
function rawFields(agent: Agent) {
  return (
    agent as unknown as {
      __getOverridableFields?: () => { instructions?: unknown; tools?: unknown };
    }
  ).__getOverridableFields?.();
}
function rawInstructions(agent: Agent): Instructions | Function | undefined {
  return rawFields(agent)?.instructions as Instructions | Function | undefined;
}
/** Mastra's own conversion of static instructions to the system text it sends. */
function instructionsText(instructions: Instructions): string {
  const text = (message: string | { content?: unknown }) =>
    typeof message === "string"
      ? message
      : typeof message.content === "string"
        ? message.content
        : "";
  return Array.isArray(instructions)
    ? instructions.map(text).filter(Boolean).join("\n\n")
    : text(instructions);
}
/** Prompt names allow `[A-Za-z0-9._/-]`; Mastra ids default to the agent's display name. */
const promptName = (agent: Agent) => `${agent.id.replace(/[^A-Za-z0-9._-]/g, "-")}/instructions`;

/**
 * The agent's instructions as a prompt: static text is plain (Mastra does not template it),
 * a function is dynamic with its source as the template. Undefined when there is none.
 */
export function instructionsPrompt(agent: Agent, origin = ""): DiscoveredPrompt | undefined {
  const raw = rawInstructions(agent);
  if (typeof raw === "function")
    return {
      name: promptName(agent),
      format: PromptFormat.DYNAMIC,
      template: raw.toString(),
      origin: `${origin}.instructions`,
    };
  const template = raw === undefined ? "" : instructionsText(raw);
  if (!template) return undefined;
  return {
    name: promptName(agent),
    format: PromptFormat.PLAIN,
    template,
    origin: `${origin}.instructions`,
  };
}
/** Stamp of the agent's dynamic instructions, or undefined when they are static. */
export function dynamicInstructionsStamp(agent: Agent) {
  const prompt = instructionsPrompt(agent);
  if (prompt?.format !== PromptFormat.DYNAMIC) return undefined;
  return { name: prompt.name, hash: promptHash(prompt.template, {}, canonicalJson({})) };
}

async function discoverAgent(agent: Agent, origin: string): Promise<Required<Discovery>> {
  const discovery: Required<Discovery> = { prompts: [], skills: [], tools: [], warnings: [] };
  if (rawInstructions(agent) === undefined)
    discovery.warnings.push(`Mastra agent ${agent.id}: instructions could not be read (${origin})`);
  const prompt = instructionsPrompt(agent, origin);
  if (prompt) discovery.prompts.push(prompt);
  // Tools given as a function are resolved per call and cannot be declared.
  const tools = rawFields(agent)?.tools;
  if (tools && typeof tools === "object")
    discovery.tools = discoverTools(tools as Record<string, AnyTool>, {}, `${origin}.tools`);
  try {
    for (const skill of await agent.listSkills()) {
      // Paths are resolved as Mastra resolves them: relative to the working directory.
      const dir = resolve(process.cwd(), skill.path);
      if (existsSync(join(dir, "SKILL.md"))) {
        discovery.skills.push({ dir, origin: `${origin}.skills` });
        continue;
      }
      const inline = await agent.getSkill(skill.name);
      if (!inline) continue;
      discovery.skills.push(await inlineSkill(agent, inline, `${origin}.skills`));
    }
  } catch (error) {
    discovery.warnings.push(
      `Mastra agent ${agent.id}: skills could not be listed (${(error as Error).message})`,
    );
  }
  return discovery;
}
/** A `createSkill()` skill: SKILL.md rebuilt from its fields, references under `references/`. */
async function inlineSkill(
  agent: Agent,
  skill: NonNullable<Awaited<ReturnType<Agent["getSkill"]>>>,
  origin: string,
): Promise<DiscoveredSkill> {
  const line = (value: string) => value.replace(/\s+/g, " ").trim();
  const files = [
    {
      path: "SKILL.md",
      content: `---\nname: ${skill.name}\ndescription: ${line(skill.description)}\n---\n\n${skill.instructions.trim()}\n`,
    },
  ];
  // Reference contents are only reachable through the agent's resolved skills.
  const resolved = await (
    agent as unknown as {
      resolveSkills?: () => Promise<
        { getReference(name: string, path: string): Promise<string | null> } | undefined
      >;
    }
  ).resolveSkills?.();
  for (const reference of skill.references) {
    const path = `references/${reference}`;
    const content = await resolved?.getReference(skill.name, path);
    if (content != null) files.push({ path, content });
  }
  return { name: skill.name, files, origin };
}

/**
 * `tilde deploy` calls this for each export of the entry module. A `Mastra` instance
 * contributes each registered agent; an `Agent` its instructions (`<agent id>/instructions`),
 * skills (directories, and `createSkill()` skills as files) and static tools; a `defineTools`
 * value holding Mastra tools its tools.
 */
export async function discover(
  value: unknown,
  context: DiscoveryContext,
): Promise<Discovery | undefined> {
  if (
    isBundledTools(value) &&
    !!value.tools &&
    typeof value.tools === "object" &&
    Object.values(value.tools).some((tool) => tool instanceof Tool)
  ) {
    const tools = value.tools as Record<string, AnyTool>;
    return { tools: discoverTools(tools, value.options, `${context.origin}.tools`) };
  }
  if (isAgent(value)) return discoverAgent(value, context.origin);
  if (!isMastra(value)) return undefined;
  const merged: Required<Discovery> = { prompts: [], skills: [], tools: [], warnings: [] };
  for (const [key, agent] of Object.entries(value.listAgents())) {
    const found = await discoverAgent(agent as Agent, `${context.origin}.agents.${key}`);
    merged.prompts.push(...found.prompts);
    merged.skills.push(...found.skills);
    merged.tools.push(...found.tools);
    merged.warnings.push(...found.warnings);
  }
  return merged;
}
