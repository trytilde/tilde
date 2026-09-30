import { existsSync } from "node:fs";
import { chmod, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { declaration } from "./invocation.js";
import type { Tool } from "./index.js";
import type { Client as RpcClient } from "@connectrpc/connect";
import type { SkillService } from "@trytilde/contracts/tilde/runtime/v1/skills_pb.js";

export type SkillFileInfo = {
  path: string;
  mediaType: string;
  sizeBytes: bigint;
  /** Written with mode 0o755 when materialized. */
  executable: boolean;
};
export type SkillSummary = {
  name: string;
  /** The skill source's slug; `source/name` addresses the skill when names collide. */
  source: string;
  description: string;
  versionId: string;
  files: SkillFileInfo[];
  /** Shipped by this invocation's deployment: its files already sit beside the code. */
  deployed: boolean;
};
/** Text arrives inline; other files as a short-lived download URL. */
export type SkillFileBody = { content?: string; downloadUrl?: string; mediaType: string };
export type SkillSourceSummary = { slug: string; name: string; skills: SkillSummary[] };
export type SkillsClient = {
  /** Names, descriptions and file lists; read a skill's files when the task calls for it. */
  list(agentId?: string): Promise<SkillSummary[]>;
  /** `name` or `source/name`; defaults to the skill's SKILL.md. */
  read(name: string, path?: string, agentId?: string): Promise<SkillFileBody>;
  /** Write every skill under `directory/<name>/…` for harnesses that load skills from disk. */
  materialize(directory: string): Promise<string[]>;
  /**
   * The skills the deployment did not ship (`deployed === false`: assigned in the registry), in
   * one folder per skill under a directory this process keeps for the agent. Only what changed
   * since the last call is fetched: new and newer versions are downloaded, removed skills
   * deleted. Returns that directory; it holds no skill folders when there are none.
   */
  directory(): Promise<string>;
  /** A block listing the skills for a system prompt, in the shape agent harnesses expect. */
  summary(): Promise<string>;
  /**
   * `list_skills` and `read_skill` as Tilde tools, with `summary()` in the instructions, for
   * frameworks without native skills. Convert them like channel tools.
   */
  tools(): Record<string, Tool>;
  /** Every skill source and its skills; needs agents.edit_skills. */
  sources(): Promise<SkillSourceSummary[]>;
  /** Give an agent (this one by default) a whole source by slug, or one skill as `source/name`. */
  assign(target: { source: string } | { skill: string }, agentId?: string): Promise<void>;
  unassign(target: { source: string } | { skill: string }, agentId?: string): Promise<void>;
  /** Create or revise a skill in an editor source; needs skills.edit on the source. */
  write(
    source: string,
    name: string,
    files: { path: string; content?: string; data?: Uint8Array; executable?: boolean }[],
    message?: string,
  ): Promise<{ versionId: string; number: number; created: boolean }>;
  /** Re-sync a git or catalog source; needs skills.edit on it. */
  sync(source: string): Promise<{ commitSha: string; syncError: string }>;
};

/**
 * Registry skills this process holds for one agent: a folder per skill under
 * `$TILDE_SKILLS_DIR` (the OS temp directory by default, which lasts as long as a container),
 * and in memory the version each folder holds, so an unchanged invocation touches no files.
 * `.versions.json` keeps that map for the next process.
 */
type LocalSkills = { root: string; versions: Map<string, string>; queue: Promise<unknown> };
const locals = new Map<string, LocalSkills>();
async function local(agentId: string): Promise<LocalSkills> {
  let held = locals.get(agentId);
  if (held) return held;
  const root = join(process.env.TILDE_SKILLS_DIR || tmpdir(), "tilde-skills", agentId || "agent");
  const versions = new Map<string, string>();
  try {
    const saved = JSON.parse(await readFile(join(root, ".versions.json"), "utf8")) as Record<
      string,
      string
    >;
    for (const [folder, version] of Object.entries(saved))
      if (existsSync(join(root, folder))) versions.set(folder, version);
  } catch {
    // First run, or an unreadable manifest: every skill downloads again.
  }
  held = locals.get(agentId) ?? { root, versions, queue: Promise.resolve() };
  locals.set(agentId, held);
  return held;
}

export function createSkillsClient(
  client: RpcClient<typeof SkillService>,
  options: () => { headers: Headers; signal: AbortSignal },
  /** The invocation's agent and the skills the engine pushed with it, when it did. */
  invocation: { agentId: string; skills?: SkillSummary[] } = { agentId: "" },
): SkillsClient {
  // Pushed skills answer for this agent without a call; other agents are always listed.
  const list = async (agentId = "") =>
    !agentId && invocation.skills
      ? invocation.skills
      : (await client.listSkills({ agentId }, options())).skills;
  const read = async (name: string, path = "SKILL.md", agentId = "") => {
    const file = await client.readSkillFile({ name, path, agentId }, options());
    return file.downloadUrl
      ? { downloadUrl: file.downloadUrl, mediaType: file.mediaType }
      : { content: file.content, mediaType: file.mediaType };
  };
  /** Folder per skill: its name, or `<source>-<name>` for names two sources share. */
  const folders = (skills: SkillSummary[]) => {
    const repeated = new Set(skills.map((s) => s.name).filter((n, i, all) => all.indexOf(n) !== i));
    return skills.map((skill) => ({
      skill,
      address: repeated.has(skill.name) ? `${skill.source}/${skill.name}` : skill.name,
      folder: repeated.has(skill.name) ? `${skill.source}-${skill.name}` : skill.name,
    }));
  };
  const writeSkill = async (skill: SkillSummary, address: string, dir: string) => {
    const written: string[] = [];
    for (const file of skill.files) {
      const target = resolve(dir, file.path);
      if (!target.startsWith(dir + sep)) throw new Error(`Skill path escapes ${dir}`);
      await mkdir(join(target, ".."), { recursive: true });
      const body = await read(address, file.path);
      if (body.content !== undefined) await writeFile(target, body.content);
      else {
        const response = await fetch(body.downloadUrl!, { signal: options().signal });
        if (!response.ok) throw new Error(`Unable to download ${address}/${file.path}`);
        await writeFile(target, new Uint8Array(await response.arrayBuffer()));
      }
      if (file.executable) await chmod(target, 0o755);
      written.push(target);
    }
    return written;
  };
  const writeAll = async (skills: SkillSummary[], root: string) => {
    const written: string[] = [];
    for (const { skill, address, folder } of folders(skills))
      written.push(...(await writeSkill(skill, address, resolve(root, folder))));
    return written;
  };
  /** Bring the agent's folder to these skills: fetch new and changed versions, drop the rest. */
  const update = async (skills: SkillSummary[]) => {
    const held = await local(invocation.agentId);
    const run = held.queue.then(async () => {
      const wanted = folders(skills);
      let changed = false;
      await mkdir(held.root, { recursive: true });
      for (const { skill, address, folder } of wanted) {
        if (held.versions.get(folder) === skill.versionId) continue;
        // Written beside the target and swapped in, so a reader never sees half a skill.
        const staging = await mkdtemp(join(held.root, `.${folder}-`));
        try {
          await writeSkill(skill, address, staging);
          await rm(join(held.root, folder), { recursive: true, force: true });
          await rename(staging, join(held.root, folder));
        } catch (error) {
          await rm(staging, { recursive: true, force: true });
          throw error;
        }
        held.versions.set(folder, skill.versionId);
        changed = true;
      }
      const kept = new Set(wanted.map((w) => w.folder));
      for (const folder of [...held.versions.keys()].filter((f) => !kept.has(f))) {
        await rm(join(held.root, folder), { recursive: true, force: true });
        held.versions.delete(folder);
        changed = true;
      }
      if (changed)
        await writeFile(
          join(held.root, ".versions.json"),
          JSON.stringify(Object.fromEntries(held.versions)),
        );
      return held.root;
    });
    // One update at a time per agent; a failed one does not block the next.
    held.queue = run.catch(() => {});
    return run;
  };
  return {
    list,
    read,
    materialize: async (directory) => writeAll(await list(), resolve(directory)),
    directory: async () => update((await list()).filter((skill) => !skill.deployed)),
    async summary() {
      const skills = await list();
      if (!skills.length) return "";
      return [
        "You have these skills. Read a skill's SKILL.md with the read_skill tool before using it.",
        ...skills.map((skill) => `- ${skill.name}: ${skill.description}`),
      ].join("\n");
    },
    tools: () => ({
      list_skills: {
        description: "List your skills: name, source, description and files.",
        inputSchema: { type: "object", properties: {}, additionalProperties: false },
        execute: async () =>
          (await list()).map((skill) => ({
            name: skill.name,
            source: skill.source,
            description: skill.description,
            files: skill.files.map((file) => file.path),
          })),
      },
      read_skill: {
        description:
          "Read one of your skills' files, SKILL.md by default. Use `source/name` when two sources share a name.",
        inputSchema: {
          type: "object",
          properties: { name: { type: "string" }, path: { type: "string" } },
          required: ["name"],
          additionalProperties: false,
        },
        execute: async (input) => {
          const { name, path } = input as { name: string; path?: string };
          return read(name, path || undefined);
        },
      },
    }),
    sources: async () => (await client.listSkillSources({}, options())).sources,
    async assign(target, agentId = "") {
      await client.assignSkill({ agentId, ...target }, options());
    },
    async unassign(target, agentId = "") {
      await client.unassignSkill({ agentId, ...target }, options());
    },
    async write(source, name, files, message = "") {
      const response = await client.writeSkill({ source, name, files, message }, options());
      return { versionId: response.versionId, number: response.number, created: response.created };
    },
    async sync(source) {
      const response = await client.syncSkillSource({ source }, options());
      return { commitSha: response.commitSha, syncError: response.syncError };
    },
  };
}

/** A directory whose subfolders each hold a skill (a `SKILL.md` and its files). */
export type SkillsDefinition = { readonly [declaration]: "skills"; readonly dir: string };
export type SkillFileInput = {
  path: string;
  content?: string;
  data?: Uint8Array;
  executable?: boolean;
};
/** One skill written in code; its `SKILL.md` is generated from the fields. */
export type SkillDefinition = {
  readonly [declaration]: "skill";
  readonly name: string;
  readonly files: readonly SkillFileInput[];
};

/**
 * Ship every skill under `dir` with the deployment. Export the result from the entry module;
 * `tilde deploy` reads the files. A relative path is resolved against the working directory
 * now, so prefer `new URL("../skills", import.meta.url)`.
 */
export function defineSkills(options: { dir: string | URL }): SkillsDefinition {
  const dir = options.dir instanceof URL ? fileURLToPath(options.dir) : resolve(options.dir);
  return { [declaration]: "skills", dir };
}
/** Ship one skill written in code: `SKILL.md` gets front matter, `instructions` as its body. */
export function defineSkill(options: {
  name: string;
  description: string;
  instructions: string;
  files?: SkillFileInput[];
}): SkillDefinition {
  const line = (value: string) => value.replace(/\s+/g, " ").trim();
  const entry = `---\nname: ${line(options.name)}\ndescription: ${line(options.description)}\n---\n\n${options.instructions.trim()}\n`;
  if (options.files?.some((file) => file.path === "SKILL.md"))
    throw new Error(`Skill ${options.name} generates its own SKILL.md`);
  return {
    [declaration]: "skill",
    name: options.name,
    files: [{ path: "SKILL.md", content: entry }, ...(options.files ?? [])],
  };
}
