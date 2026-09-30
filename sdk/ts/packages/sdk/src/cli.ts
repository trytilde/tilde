#!/usr/bin/env node
/**
 * `tilde deploy [ENTRY]`: import the agent's entry module, collect the prompts, skills and
 * bundled tools it declares (see discovery.ts) and register a deployment carrying them. `--dry-run` prints the
 * declarations as protobuf JSON and contacts nothing. The entry is normally built JavaScript
 * (CI points at `dist/index.js`); a `.ts` entry relies on Node's own type stripping.
 * stdout carries only the result (the token, or JSON) so `TOKEN=$(tilde deploy)` works.
 */
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { parseArgs } from "node:util";
import { create, toJsonString } from "@bufbuild/protobuf";
import { Code, ConnectError, createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-node";
import {
  DeploymentDeclarationsSchema,
  DeploymentService,
} from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import {
  DeploymentSource,
  DeploymentTarget,
} from "@trytilde/contracts/tilde/types/v1/deployment_pb.js";
import { ToolDisplay } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import { PromptFormat } from "@trytilde/contracts/tilde/types/v1/prompt_pb.js";
import { discover, type Declarations } from "./discovery.js";
import { canonicalJson } from "./prompts.js";

const usage = `Usage: tilde deploy [ENTRY] [--agent-id ID] [--url URL] [--api-key KEY]
                    [--target gateway|sidecar|lambda] [--function-arn ARN]
                    [--external-id ID] [--label LABEL] [--dry-run] [--json]

ENTRY defaults to package.json "main", then src/index.ts or index.js.
--agent-id, --url and --api-key fall back to TILDE_AGENT_ID, TILDE_URL and TILDE_API_KEY.
--api-key is optional: open-source Tilde needs none; Tilde Cloud requires one.`;

class UsageError extends Error {}

function entryPath(given: string | undefined): string {
  const cwd = process.cwd();
  if (given) return resolve(cwd, given);
  const manifest = join(cwd, "package.json");
  if (existsSync(manifest)) {
    const main = JSON.parse(readFileSync(manifest, "utf8")).main;
    if (typeof main === "string") return resolve(cwd, main);
  }
  for (const candidate of ["src/index.ts", "index.js"])
    if (existsSync(join(cwd, candidate))) return join(cwd, candidate);
  throw new UsageError("No ENTRY given and none found (package.json main, src/index.ts, index.js)");
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

/** Text travels inline; anything else as bytes (dry run) or by the digest of an upload. */
function declarationsMessage(found: Declarations, uploaded?: Map<Uint8Array, string>) {
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
            : uploaded
              ? { case: "sha256" as const, value: uploaded.get(file.data)! }
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
  const lines = [`tilde deploy: ${relative(process.cwd(), entry) || entry}`];
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

async function deploy(argv: string[]) {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      "agent-id": { type: "string" },
      url: { type: "string" },
      "api-key": { type: "string" },
      target: { type: "string", default: "gateway" },
      "function-arn": { type: "string" },
      "external-id": { type: "string" },
      label: { type: "string" },
      "dry-run": { type: "boolean", default: false },
      json: { type: "boolean", default: false },
    },
  });
  if (positionals.length > 1) throw new UsageError("Give at most one ENTRY");
  const targets: Record<string, DeploymentTarget> = {
    gateway: DeploymentTarget.GATEWAY,
    sidecar: DeploymentTarget.SIDECAR,
    lambda: DeploymentTarget.LAMBDA,
  };
  const target = targets[values.target!];
  if (!target) throw new UsageError("--target must be gateway, sidecar or lambda");
  if ((target === DeploymentTarget.LAMBDA) !== !!values["function-arn"])
    throw new UsageError("--function-arn is required for lambda and only for lambda");

  const entry = entryPath(positionals[0]);
  if (!existsSync(entry)) throw new UsageError(`Entry ${entry} does not exist`);
  const found = await discover(entry, projectDir(entry)).catch((error: unknown) => {
    // Type stripping runs `.ts` files but does not map `./x.js` imports to `./x.ts`.
    if (/\.[mc]?ts$/.test(entry) && (error as { code?: string }).code === "ERR_MODULE_NOT_FOUND")
      throw new Error(
        `${(error as Error).message}\nPoint tilde deploy at the built JavaScript entry.`,
      );
    throw error;
  });
  inventory(entry, found);
  if (!found.prompts.length && !found.skills.length && !found.tools.length)
    throw new Error(
      "Nothing to deploy was found: export definePrompt/defineSkills/defineTools values or framework agents from the entry module",
    );
  if (values["dry-run"])
    return toJsonString(DeploymentDeclarationsSchema, declarationsMessage(found));

  const env = process.env;
  const url = values.url ?? env.TILDE_URL;
  const apiKey = values["api-key"] ?? env.TILDE_API_KEY;
  const agentId = values["agent-id"] ?? env.TILDE_AGENT_ID;
  if (!url || !agentId)
    throw new UsageError("--url and --agent-id (or TILDE_URL, TILDE_AGENT_ID) are required");
  const client = createClient(
    DeploymentService,
    createConnectTransport({
      baseUrl: url.replace(/\/+$/, ""),
      httpVersion: "1.1",
      interceptors: [
        (next) => (request) => {
          if (apiKey) request.header.set("authorization", `Bearer ${apiKey}`);
          return next(request);
        },
      ],
    }),
  );
  // Content-addressed: upload only files the engine does not already hold.
  const uploads = new Map<string, Uint8Array>();
  const digests = new Map<Uint8Array, string>();
  for (const skill of found.skills)
    for (const file of skill.files)
      if (file.text === undefined) {
        const digest = createHash("sha256").update(file.data).digest("hex");
        digests.set(file.data, digest);
        uploads.set(digest, file.data);
      }
  if (uploads.size) {
    const { sha256: missing } = await client.missingDeploymentFiles({
      agentId,
      sha256: [...uploads.keys()],
    });
    for (const digest of missing) {
      const uploaded = await client.uploadDeploymentFile({ agentId, data: uploads.get(digest)! });
      if (uploaded.sha256 !== digest)
        throw new Error("Tilde stored an upload under another digest");
    }
  }
  const github = !!env.GITHUB_ACTIONS;
  const response = await client.registerDeployment({
    agentId,
    source: env.CI || github ? DeploymentSource.CI : DeploymentSource.MANUAL,
    target,
    targetReference: values["function-arn"],
    repository: env.GITHUB_REPOSITORY || undefined,
    commitSha: env.GITHUB_SHA || undefined,
    branch: env.GITHUB_REF_NAME || undefined,
    externalId:
      values["external-id"] ??
      (github && env.GITHUB_REPOSITORY && env.GITHUB_RUN_ID
        ? `${env.GITHUB_REPOSITORY}:${env.GITHUB_RUN_ID}`
        : undefined),
    label: values.label,
    declarations: declarationsMessage(found, digests),
  });
  const deploymentId = response.deployment?.id ?? "";
  process.stderr.write(
    `Deployment ${deploymentId} ${response.created ? "registered" : "already registered (no token)"}\n`,
  );
  return values.json
    ? JSON.stringify({ deploymentId, token: response.token || null, created: response.created })
    : response.token;
}

async function main() {
  const [command, ...rest] = process.argv.slice(2);
  if (command !== "deploy") throw new UsageError(command ? `Unknown command ${command}` : usage);
  const output = await deploy(rest);
  // User modules may leave handles open (timers, sockets); exit once the result is written.
  process.stdout.write(`${output}\n`, () => process.exit(0));
}

main().catch((error: unknown) => {
  const message =
    error instanceof UsageError || (error as { code?: string })?.code?.startsWith("ERR_PARSE_ARGS")
      ? `${(error as Error).message}\n\n${usage}`
      : error instanceof ConnectError
        ? `Tilde rejected the deployment (${Code[error.code]}): ${error.rawMessage}`
        : error instanceof Error
          ? error.message
          : String(error);
  process.stderr.write(`tilde: ${message}\n`, () => process.exit(1));
});
