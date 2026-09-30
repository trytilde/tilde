import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import {
  PromptFormat,
  isBundledTools,
  type BundledToolOptions,
  type DiscoveredPrompt,
  type DiscoveredTool,
  type Discovery,
  type DiscoveryContext,
} from "@trytilde/sdk";
import type { ToolSet } from "ai";
import { tildeCallOptions } from "./invocation.js";
import { describeTool } from "./tools.js";

type SystemText = string | { content?: unknown };
type Settings = {
  instructions?: SystemText | SystemText[] | Function;
  prepareCall?: unknown;
  tools?: ToolSet;
};

// Duck-typed rather than `instanceof`: the project's `ai` may be another copy. Any `Agent`
// (the AI SDK interface) matches; only a `ToolLoopAgent` has readable settings.
const isAgent = (value: unknown): value is { id?: string; settings?: Settings } =>
  !!value &&
  typeof value === "object" &&
  (value as { version?: unknown }).version === "agent-v1" &&
  typeof (value as { generate?: unknown }).generate === "function" &&
  typeof (value as { stream?: unknown }).stream === "function";

// AI SDK tools are plain objects (`tool()` returns its argument); Mastra's are class instances.
const isToolSet = (value: unknown): value is ToolSet =>
  !!value &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  Object.values(value).every(
    (entry) =>
      !!entry &&
      Object.getPrototypeOf(entry) === Object.prototype &&
      ("inputSchema" in entry || entry.type === "provider"),
  );
/** Tools as `withTildeTools` publishes them, each at `<origin>.<name>`. */
async function discoverTools(tools: ToolSet, options: BundledToolOptions, origin: string) {
  const found: DiscoveredTool[] = [];
  for (const [name, native] of Object.entries(tools)) {
    const described = await describeTool(name, native, options);
    if (described) found.push({ ...described, origin: `${origin}.${name}` });
  }
  return found;
}

const safeName = (name: string) => name.replace(/[^A-Za-z0-9._-]/g, "-");
const text = (message: SystemText) =>
  typeof message === "string"
    ? message
    : typeof message?.content === "string"
      ? message.content
      : "";

/**
 * A `ToolLoopAgent`'s instructions as `<agent id or export name>/instructions`: a string or
 * system message is plain text (the AI SDK does not template it); several system messages
 * are sent separately, so each is its own prompt (`…/instructions/<i>`). Its `tools` are
 * declared as bundled tools.
 *
 * The AI SDK keeps the constructor settings in a TypeScript-private `settings` field with no
 * getter for instructions; this reads it (verified against ai 6.0.280) and warns when it is
 * missing.
 */
async function discoverAgent(
  agent: { id?: string; settings?: Settings },
  context: DiscoveryContext,
): Promise<Discovery> {
  const exported = context.origin.slice(context.origin.indexOf("#") + 1);
  const label = agent.id ?? exported;
  const name = `${safeName(label)}/instructions`;
  const origin = `${context.origin}.instructions`;
  const settings = agent.settings;
  if (!settings || typeof settings !== "object")
    return {
      warnings: [`AI SDK agent ${label}: instructions could not be read (${context.origin})`],
    };
  const warnings: string[] = [];
  if (settings.prepareCall && settings.prepareCall !== tildeCallOptions.prepareCall)
    warnings.push(
      `AI SDK agent ${label}: prepareCall may replace its instructions per call; the static text is registered (${context.origin})`,
    );
  const tools = settings.tools
    ? await discoverTools(settings.tools, {}, `${context.origin}.tools`)
    : [];
  const instructions = settings.instructions;
  if (typeof instructions === "function")
    return {
      prompts: [{ name, format: PromptFormat.DYNAMIC, template: instructions.toString(), origin }],
      tools,
      warnings,
    };
  const messages = (Array.isArray(instructions) ? instructions : [instructions])
    .filter((message) => message !== undefined)
    .map(text);
  const prompts: DiscoveredPrompt[] = messages
    .map((template, i) => ({
      name: messages.length > 1 ? `${name}/${i}` : name,
      format: PromptFormat.PLAIN,
      template,
      origin: messages.length > 1 ? `${origin}[${i}]` : origin,
    }))
    .filter((prompt) => prompt.template);
  return { prompts, tools, warnings };
}

/**
 * `tilde deploy` calls this for each export of the entry module: a `ToolLoopAgent`, or a
 * `defineTools` value holding an AI SDK `ToolSet`.
 */
export async function discover(
  value: unknown,
  context: DiscoveryContext,
): Promise<Discovery | undefined> {
  if (isBundledTools(value) && isToolSet(value.tools))
    return { tools: await discoverTools(value.tools, value.options, `${context.origin}.tools`) };
  return isAgent(value) ? discoverAgent(value, context) : undefined;
}

/**
 * Vercel's eve defines an agent by files under `agent/` rather than an exported object:
 * `agent/instructions.md` is a plain prompt `agent/instructions`; `agent/instructions.{ts,mjs,js}`
 * is imported and its default (or `instructions`) export read: a string is plain, a function
 * dynamic with its source as the template. `agent/skills/<name>/SKILL.md` are skills.
 */
export async function discoverProject(projectDir: string): Promise<Discovery | undefined> {
  const agent = join(projectDir, "agent");
  if (!existsSync(agent)) return undefined;
  const found: Required<Discovery> = { prompts: [], skills: [], tools: [], warnings: [] };
  const name = "agent/instructions";
  if (existsSync(join(agent, "instructions.md"))) {
    const template = readFileSync(join(agent, "instructions.md"), "utf8");
    if (template.trim())
      found.prompts.push({
        name,
        format: PromptFormat.PLAIN,
        template,
        origin: "agent/instructions.md",
      });
  } else {
    const file = ["instructions.ts", "instructions.mjs", "instructions.js"].find((file) =>
      existsSync(join(agent, file)),
    );
    if (file)
      try {
        const module = await import(pathToFileURL(join(agent, file)).href);
        const key = module.default !== undefined ? "default" : "instructions";
        const value: unknown = module[key];
        const origin = `agent/${file}#${key}`;
        if (typeof value === "function")
          found.prompts.push({
            name,
            format: PromptFormat.DYNAMIC,
            template: value.toString(),
            origin,
          });
        else if (typeof value === "string" && value.trim())
          found.prompts.push({ name, format: PromptFormat.PLAIN, template: value, origin });
        else found.warnings.push(`eve: agent/${file} exports no instructions`);
      } catch (error) {
        found.warnings.push(
          `eve: agent/${file} could not be imported (${(error as Error).message})`,
        );
      }
  }
  const skills = join(agent, "skills");
  if (
    existsSync(skills) &&
    readdirSync(skills, { withFileTypes: true }).some(
      (entry) => entry.isDirectory() && existsSync(join(skills, entry.name, "SKILL.md")),
    )
  )
    found.skills.push({ dir: skills, origin: "agent/skills" });
  return found.prompts.length || found.skills.length || found.warnings.length ? found : undefined;
}
