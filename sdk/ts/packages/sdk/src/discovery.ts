import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { readdir, readFile, stat } from "node:fs/promises";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { PromptFormat } from "@trytilde/contracts/tilde/types/v1/prompt_pb.js";
import { canonicalJson, promptHash, validPromptName } from "./prompts.js";
import { declaration } from "./invocation.js";
import type { SkillFileInput } from "./skills.js";
import type { ToolAnnotations, ToolDisplayMode } from "./index.js";

/**
 * What `tilde deploy` reads from an agent's entry module. Core recognises `definePrompt`,
 * `defineSkills` and `defineSkill` values; framework adapters (`@trytilde/sdk-<framework>-node`)
 * export a `discover` that recognises their framework's own objects and the native tools of a
 * `defineTools` value.
 */
export type DiscoveredPrompt = {
  name: string;
  format: PromptFormat;
  /** For a dynamic prompt, the function's source. */
  template: string;
  sections?: Record<string, string>;
  config?: Record<string, unknown>;
  origin: string;
};
/** A skill directory (holding `SKILL.md`) or a directory of them, or one skill written in code. */
export type DiscoveredSkill =
  | { dir: string; origin: string }
  | { name: string; files: readonly SkillFileInput[]; origin: string };
/** A bundled tool as the adapter's `withTildeTools` publishes it. */
export type DiscoveredTool = {
  name: string;
  description: string;
  summary?: string;
  display?: ToolDisplayMode;
  annotations?: ToolAnnotations;
  inputSchema: Record<string, unknown>;
  outputSchema?: Record<string, unknown>;
  origin: string;
};
export type Discovery = {
  prompts?: DiscoveredPrompt[];
  skills?: DiscoveredSkill[];
  tools?: DiscoveredTool[];
  /** For example a framework agent whose prompts could not be read. */
  warnings?: string[];
};
/** `origin` locates the value, e.g. `dist/index.js#agent`; paths are relative to `projectDir`. */
export type DiscoveryContext = { origin: string; projectDir: string };
export type Discoverer = (
  value: unknown,
  context: DiscoveryContext,
) => Discovery | undefined | Promise<Discovery | undefined>;

/**
 * Frameworks defined by files rather than exported objects (Vercel's eve: `agent/…`) are read
 * from a project directory: the entry's package root, and the working directory when it differs.
 */
export type ProjectDiscoverer = (
  projectDir: string,
) => Discovery | undefined | Promise<Discovery | undefined>;
type Adapter = { discover?: Discoverer; discoverProject?: ProjectDiscoverer };

/** Adapter packages tried for `discover`/`discoverProject` exports; missing ones are skipped. */
const ADAPTERS = [
  "@trytilde/sdk-mastra-node",
  "@trytilde/sdk-vercel-ai-node",
  "@trytilde/sdk-langchain-node",
  "@trytilde/sdk-openai-agents-node",
];

export type SkillFile = { path: string; executable: boolean; data: Uint8Array; text?: string };
export type Declarations = {
  prompts: (DiscoveredPrompt & { hash: string })[];
  skills: { name: string; files: SkillFile[]; origin: string }[];
  tools: DiscoveredTool[];
  warnings: string[];
};

const INLINE_TEXT_BYTES = 256 * 1024;
const utf8 = new TextDecoder("utf-8", { fatal: true });
function skillFile(path: string, data: Uint8Array, executable: boolean): SkillFile {
  let text: string | undefined;
  // SKILL.md (capped at 1 MiB) always travels inline: the server reads its front matter.
  if (path === "SKILL.md" || data.length <= INLINE_TEXT_BYTES)
    try {
      text = utf8.decode(data);
    } catch {
      // Binary: travels as bytes.
    }
  return { path, executable, data, text };
}
/** `name:` from SKILL.md front matter, as `crates/tilde/src/skills/package.rs` reads it. */
function frontMatterName(skillMd: string): string | undefined {
  const lines = skillMd.split(/\r?\n/);
  if (lines[0]?.trim() !== "---") return undefined;
  for (const line of lines.slice(1)) {
    if (line.trim() === "---") break;
    if (line.startsWith("name:"))
      return line
        .slice(5)
        .trim()
        .replace(/^["']|["']$/g, "");
  }
  return undefined;
}
async function readSkillDir(dir: string, origin: string) {
  const files: SkillFile[] = [];
  const walk = async (folder: string) => {
    for (const entry of await readdir(folder, { withFileTypes: true })) {
      if (entry.name === ".git") continue;
      const path = join(folder, entry.name);
      const info = await stat(path);
      if (info.isDirectory()) await walk(path);
      else if (info.isFile())
        files.push(
          skillFile(
            relative(dir, path).split("\\").join("/"),
            new Uint8Array(await readFile(path)),
            (info.mode & 0o111) !== 0,
          ),
        );
    }
  };
  await walk(dir);
  const entry = files.find((file) => file.path === "SKILL.md");
  if (!entry) throw new Error(`Skill directory ${dir} has no SKILL.md (${origin})`);
  files.sort((a, b) => (a.path < b.path ? -1 : 1));
  return { name: frontMatterName(entry.text ?? "") ?? basename(dir), files, origin };
}

function core(value: unknown, origin: string): Discovery | undefined {
  if (!value || typeof value !== "object") return undefined;
  const kind = (value as Record<symbol, unknown>)[declaration];
  if (kind === "prompt") {
    const p = value as DiscoveredPrompt;
    const prompt: DiscoveredPrompt = {
      name: p.name,
      format: p.format,
      template: p.template,
      sections: p.sections,
      config: p.config,
      origin,
    };
    return { prompts: [prompt] };
  }
  if (kind === "skills") {
    const dir = (value as { dir: string }).dir;
    if (!existsSync(dir))
      throw new Error(`defineSkills directory ${dir} does not exist (${origin})`);
    return { skills: [{ dir, origin }] };
  }
  if (kind === "skill") {
    const skill = value as { name: string; files: SkillFileInput[] };
    return { skills: [{ name: skill.name, files: skill.files, origin }] };
  }
  return undefined;
}
const isPlain = (value: unknown): value is Record<string, unknown> =>
  !!value &&
  typeof value === "object" &&
  [Object.prototype, null].includes(Object.getPrototypeOf(value));

/** Find an adapter installed for the project by walking up its `node_modules` folders. */
async function loadAdapter(name: string, from: string): Promise<Adapter | undefined> {
  for (let dir = from; ; dir = dirname(dir)) {
    const root = join(dir, "node_modules", name);
    if (existsSync(join(root, "package.json"))) {
      const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
      const exported = manifest.exports?.["."];
      const main =
        (typeof exported === "string" ? exported : (exported?.import ?? exported?.default)) ??
        manifest.module ??
        manifest.main ??
        "index.js";
      const adapter = await import(pathToFileURL(resolve(root, main)).href);
      return {
        discover: typeof adapter.discover === "function" ? adapter.discover : undefined,
        discoverProject:
          typeof adapter.discoverProject === "function" ? adapter.discoverProject : undefined,
      };
    }
    if (dirname(dir) === dir) return undefined;
  }
}

/**
 * Import `entry` with `TILDE_DISCOVERY=1` (so `connectAgent` starts nothing) and collect what
 * its exports declare: each export, and one level into plain objects and arrays.
 */
export async function discover(entry: string, projectDir: string): Promise<Declarations> {
  process.env.TILDE_DISCOVERY = "1";
  const module = (await import(pathToFileURL(entry).href)) as Record<string, unknown>;
  const adapters: Discoverer[] = [];
  const found: { discovery: Discovery; origin: string }[] = [];
  for (const name of ADAPTERS) {
    const adapter = await loadAdapter(name, dirname(entry));
    if (adapter?.discover) adapters.push(adapter.discover);
    for (const dir of new Set([projectDir, process.cwd()])) {
      const discovery = await adapter?.discoverProject?.(dir);
      if (discovery) found.push({ discovery, origin: dir });
    }
  }
  const file = relative(projectDir, entry).split("\\").join("/");
  const visit = async (value: unknown, path: string) => {
    const origin = `${file}#${path}`;
    const own = core(value, origin);
    if (own) return found.push({ discovery: own, origin });
    for (const adapter of adapters) {
      const discovery = await adapter(value, { origin, projectDir });
      if (discovery) return found.push({ discovery, origin });
    }
    if ((value as Record<symbol, unknown> | undefined)?.[declaration] === "tools")
      return found.push({
        discovery: {
          warnings: [
            `defineTools value's tools are not recognised by an installed Tilde framework adapter (${origin})`,
          ],
        },
        origin,
      });
    return undefined;
  };
  for (const [name, value] of Object.entries(module)) {
    if (!(await visit(value, name))) {
      if (Array.isArray(value))
        for (const [i, item] of value.entries()) await visit(item, `${name}[${i}]`);
      else if (isPlain(value))
        for (const [key, item] of Object.entries(value)) await visit(item, `${name}.${key}`);
    }
  }

  const declarations: Declarations = { prompts: [], skills: [], tools: [], warnings: [] };
  const prompts = new Map<string, Declarations["prompts"][number]>();
  const skills = new Map<string, { skill: Declarations["skills"][number]; digest: string }>();
  const tools = new Map<string, { tool: DiscoveredTool; digest: string }>();
  for (const { discovery } of found) {
    declarations.warnings.push(...(discovery.warnings ?? []));
    for (const prompt of discovery.prompts ?? []) {
      if (!validPromptName(prompt.name))
        throw new Error(`Prompt name ${prompt.name} is invalid (${prompt.origin})`);
      const sections = prompt.sections ?? {};
      const hash = promptHash(prompt.template, sections, canonicalJson(prompt.config ?? {}));
      const previous = prompts.get(prompt.name);
      if (previous && previous.hash !== hash)
        throw new Error(
          `Prompt ${prompt.name} is declared twice with different content (${previous.origin}, ${prompt.origin})`,
        );
      if (!previous) prompts.set(prompt.name, { ...prompt, sections, hash });
    }
    for (const tool of discovery.tools ?? []) {
      const { origin, ...definition } = tool;
      const digest = canonicalJson(definition);
      const previous = tools.get(tool.name);
      if (previous && previous.digest !== digest)
        throw new Error(
          `Tool ${tool.name} is declared twice with different definitions (${previous.tool.origin}, ${origin})`,
        );
      if (!previous) tools.set(tool.name, { tool, digest });
    }
    for (const found of discovery.skills ?? []) {
      const expanded =
        "dir" in found
          ? await readSkillDirs(found.dir, found.origin)
          : [
              {
                name: found.name,
                origin: found.origin,
                files: found.files.map((f) =>
                  skillFile(
                    f.path,
                    f.data ?? new TextEncoder().encode(f.content ?? ""),
                    f.executable ?? false,
                  ),
                ),
              },
            ];
      for (const skill of expanded) {
        const digest = createHash("sha256")
          .update(
            JSON.stringify(
              skill.files.map((f) => [
                f.path,
                f.executable,
                createHash("sha256").update(f.data).digest("hex"),
              ]),
            ),
          )
          .digest("hex");
        const previous = skills.get(skill.name);
        if (previous && previous.digest !== digest)
          throw new Error(
            `Skill ${skill.name} is declared twice with different files (${previous.skill.origin}, ${skill.origin})`,
          );
        if (!previous) skills.set(skill.name, { skill, digest });
      }
    }
  }
  declarations.warnings = [...new Set(declarations.warnings)];
  declarations.prompts = [...prompts.values()];
  declarations.skills = [...skills.values()].map((entry) => entry.skill);
  declarations.tools = [...tools.values()].map((entry) => entry.tool);
  return declarations;
}
/** A skill directory, or (as `defineSkills` takes) a directory of skill directories. */
async function readSkillDirs(dir: string, origin: string) {
  if (existsSync(join(dir, "SKILL.md"))) return [await readSkillDir(dir, origin)];
  const skills = [];
  for (const entry of await readdir(dir, { withFileTypes: true }))
    if (entry.isDirectory() && existsSync(join(dir, entry.name, "SKILL.md")))
      skills.push(await readSkillDir(join(dir, entry.name), origin));
  if (!skills.length)
    throw new Error(`No skills (folders with a SKILL.md) under ${dir} (${origin})`);
  return skills;
}
