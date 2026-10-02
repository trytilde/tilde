#!/usr/bin/env node
/**
 * `tilde-declarations [ENTRY]`: import the agent's entry module, collect the prompts, skills and
 * bundled tools it declares (see discovery.ts) and print them as protobuf JSON on stdout. The
 * inventory goes to stderr, so stdout carries only the JSON.
 *
 * Only this runtime can read declarations out of an agent's own modules, so this is the half of
 * `tilde deploy` that has to live in the SDK. The `tilde` CLI runs it and owns everything after:
 * uploading skill files and registering the deployment, once, for every language.
 *
 * The entry is normally built JavaScript (`dist/index.js`); a `.ts` entry relies on Node's own
 * type stripping.
 */
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { parseArgs } from "node:util";
import { create, toJsonString } from "@bufbuild/protobuf";
import { DeploymentDeclarationsSchema } from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import { ToolDisplay } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import { PromptFormat } from "@trytilde/contracts/tilde/types/v1/prompt_pb.js";
import { discover, type Declarations } from "./discovery.js";
import { canonicalJson } from "./prompts.js";

const usage = `Usage: tilde-declarations [ENTRY] [--allow-empty]

Prints the prompts, skills and bundled tools ENTRY declares, as protobuf JSON.
ENTRY defaults to package.json "main", then dist/index.js, src/index.ts or index.js.
--allow-empty prints empty declarations instead of failing, for an agent that declares nothing.
Run \`tilde deploy\` to register a deployment carrying them.`;

class UsageError extends Error {}

function entryPath(given: string | undefined): string {
  const cwd = process.cwd();
  if (given) return resolve(cwd, given);
  const manifest = join(cwd, "package.json");
  if (existsSync(manifest)) {
    const main = JSON.parse(readFileSync(manifest, "utf8")).main;
    if (typeof main === "string") return resolve(cwd, main);
  }
  for (const candidate of ["dist/index.js", "src/index.ts", "index.js"])
    if (existsSync(join(cwd, candidate))) return join(cwd, candidate);
  throw new UsageError(
    "No ENTRY given and none found (package.json main, dist/index.js, src/index.ts, index.js)",
  );
}
/** The nearest folder with a package.json above the entry: origins are relative to it. */
function projectDir(entry: string): string {
  for (let dir = dirname(entry); ; dir = dirname(dir)) {
    if (existsSync(join(dir, "package.json"))) return dir;
    if (dirname(dir) === dir) return process.cwd();
  }
}

const DISPLAY = {
  full: ToolDisplay.FULL,
  summary: ToolDisplay.SUMMARY,
  hidden: ToolDisplay.HIDDEN,
} as const;

/** Text travels inline; anything else as bytes, which `tilde deploy` turns into uploads. */
function declarationsMessage(found: Declarations) {
  return create(DeploymentDeclarationsSchema, {
    prompts: found.prompts.map((prompt) => ({
      name: prompt.name,
      format: prompt.format,
      template: prompt.template,
      sections: Object.keys(prompt.sections ?? {})
        .sort()
        .map((name) => ({ name, content: prompt.sections![name] })),
      config: canonicalJson(prompt.config ?? {}),
      hash: prompt.hash,
      origin: prompt.origin,
    })),
    skills: found.skills.map((skill) => ({
      name: skill.name,
      origin: skill.origin,
      files: skill.files.map((file) => ({
        path: file.path,
        executable: file.executable,
        body:
          file.text !== undefined
            ? { case: "content" as const, value: file.text }
            : { case: "data" as const, value: file.data },
      })),
    })),
    tools: found.tools.map((tool) => ({
      name: tool.name,
      description: tool.description,
      summary: tool.summary ?? "",
      inputSchemaJson: JSON.stringify(tool.inputSchema),
      outputSchemaJson: tool.outputSchema ? JSON.stringify(tool.outputSchema) : "",
      annotations: tool.annotations,
      display: tool.display ? DISPLAY[tool.display] : ToolDisplay.UNSPECIFIED,
      origin: tool.origin,
    })),
  });
}
function inventory(entry: string, found: Declarations) {
  const lines = [`tilde declarations: ${relative(process.cwd(), entry) || entry}`];
  for (const prompt of found.prompts)
    lines.push(
      `  prompt  ${prompt.name}  ${PromptFormat[prompt.format].toLowerCase()}  ${prompt.origin}`,
    );
  for (const skill of found.skills)
    lines.push(`  skill   ${skill.name}  ${skill.files.length} file(s)  ${skill.origin}`);
  for (const tool of found.tools) lines.push(`  tool    ${tool.name}  ${tool.origin}`);
  for (const warning of found.warnings) lines.push(`  warning: ${warning}`);
  process.stderr.write(`${lines.join("\n")}\n`);
}

async function read(argv: string[]) {
  const { positionals, values } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: { "allow-empty": { type: "boolean", default: false } },
  });
  if (positionals.length > 1) throw new UsageError("Give at most one ENTRY");

  const entry = entryPath(positionals[0]);
  if (!existsSync(entry)) throw new UsageError(`Entry ${entry} does not exist`);
  const found = await discover(entry, projectDir(entry)).catch((error: unknown) => {
    // Type stripping runs `.ts` files but does not map `./x.js` imports to `./x.ts`.
    if (/\.[mc]?ts$/.test(entry) && (error as { code?: string }).code === "ERR_MODULE_NOT_FOUND")
      throw new Error(`${(error as Error).message}\nPoint tilde at the built JavaScript entry.`);
    throw error;
  });
  inventory(entry, found);
  if (
    !values["allow-empty"] &&
    !found.prompts.length &&
    !found.skills.length &&
    !found.tools.length
  )
    throw new Error(
      "Nothing to deploy was found: export definePrompt/defineSkills/defineTools values or framework agents from the entry module",
    );
  return toJsonString(DeploymentDeclarationsSchema, declarationsMessage(found));
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv[0] === "--help" || argv[0] === "-h") {
    process.stdout.write(`${usage}\n`);
    return;
  }
  const output = await read(argv);
  // User modules may leave handles open (timers, sockets); exit once the result is written.
  process.stdout.write(`${output}\n`, () => process.exit(0));
}

main().catch((error: unknown) => {
  const message =
    error instanceof UsageError || (error as { code?: string })?.code?.startsWith("ERR_PARSE_ARGS")
      ? `${(error as Error).message}\n\n${usage}`
      : error instanceof Error
        ? error.message
        : String(error);
  process.stderr.write(`tilde-declarations: ${message}\n`, () => process.exit(1));
});
