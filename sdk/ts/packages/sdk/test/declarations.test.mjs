import test from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { fromJsonString } from "@bufbuild/protobuf";
import { DeploymentDeclarationsSchema } from "@trytilde/contracts/tilde/management/v1/deployments_pb.js";
import { ToolDisplay } from "@trytilde/contracts/tilde/types/v1/chat_pb.js";
import { PromptFormat, promptHash } from "../dist/index.js";

const cli = new URL("../dist/cli.js", import.meta.url).pathname;
const sdk = new URL("../dist/index.js", import.meta.url).href;
const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0xff, 0x00]);

/** An agent project: core declarations, a framework object and a stand-in Mastra adapter. */
async function project() {
  const dir = await mkdtemp(join(tmpdir(), "tilde-declarations-"));
  await writeFile(join(dir, "package.json"), '{"type":"module","main":"src/index.mjs"}');
  await mkdir(join(dir, "src"));
  await writeFile(
    join(dir, "src", "index.mjs"),
    `import { connectAgent, definePrompt, defineSkill, defineSkills, defineTools } from ${JSON.stringify(sdk)};
export const respond = definePrompt("respond", { template: "Hi {{name}}", config: { temperature: 0 } });
export const skills = defineSkills({ dir: new URL("../skills", import.meta.url) });
export const more = { faq: defineSkill({ name: "faq", description: "FAQ", instructions: "Answer." }) };
export const agent = { framework: "fake" };
export const same = agent;
export const clash = process.env.CLASH ? definePrompt("respond", { template: "Other" }) : undefined;
const ping = { fake: true, description: "Ping" };
export const bundledTools = defineTools({ ping }, { ping: { summary: "Pinged", display: "hidden" } });
// The same tool declared again is kept once; another definition under its name is a conflict.
export const again = defineTools({ ping }, { ping: { summary: "Pinged", display: "hidden" } });
export const toolClash = process.env.TOOL_CLASH ? defineTools({ ping: { fake: true, description: "Other" } }) : undefined;
export const unknownTools = defineTools([() => {}]);
// Without TILDE_DISCOVERY this would throw: no gateway URL or token is configured.
connectAgent({ run: async () => {} });
`,
  );
  const skill = join(dir, "skills", "greet");
  await mkdir(join(skill, "scripts"), { recursive: true });
  await writeFile(join(skill, "SKILL.md"), "---\nname: greet\ndescription: Greets\n---\n\nHi.\n");
  await writeFile(join(skill, "scripts", "run.sh"), "#!/bin/sh\necho hi\n");
  await chmod(join(skill, "scripts", "run.sh"), 0o755);
  await writeFile(join(skill, "logo.png"), png);
  const adapter = join(dir, "node_modules", "@trytilde", "sdk-mastra-node");
  await mkdir(adapter, { recursive: true });
  await writeFile(
    join(adapter, "package.json"),
    '{"name":"@trytilde/sdk-mastra-node","type":"module","exports":{".":{"import":"./index.js"}}}',
  );
  await writeFile(
    join(adapter, "index.js"),
    `export function discover(value, { origin }) {
  if (value?.[Symbol.for("tilde.declaration")] === "tools" && value.tools.ping?.fake)
    return {
      tools: [{
        name: "ping",
        description: value.tools.ping.description,
        ...value.options.ping,
        inputSchema: { type: "object" },
        outputSchema: { type: "string" },
        origin: origin + ".tools.ping",
      }],
    };
  if (value?.framework !== "fake") return undefined;
  return {
    prompts: [{ name: "fake/instructions", format: 1, template: "Be plain.", origin: origin + ".instructions" }],
    warnings: ["fake agent has no skills"],
  };
}
`,
  );
  return dir;
}
async function run(dir, args, env = {}) {
  try {
    const { stdout, stderr } = await promisify(execFile)(process.execPath, [cli, ...args], {
      cwd: dir,
      env: { PATH: process.env.PATH, ...env },
    });
    return { code: 0, stdout, stderr };
  } catch (error) {
    return { code: error.code, stdout: error.stdout, stderr: error.stderr };
  }
}

test("tilde-declarations reports every prompt, skill and tool the entry declares", async () => {
  const dir = await project();

  const dry = await run(dir, ["src/index.mjs"]);
  assert.equal(dry.code, 0, dry.stderr);
  assert.match(
    dry.stderr,
    /prompt {2}fake\/instructions {2}plain {2}src\/index\.mjs#agent\.instructions/,
  );
  assert.match(dry.stderr, /warning: fake agent has no skills/);
  assert.match(dry.stderr, /tool {4}ping {2}src\/index\.mjs#again\.tools\.ping/);
  assert.match(
    dry.stderr,
    /warning: defineTools value's tools are not recognised by an installed Tilde framework adapter \(src\/index\.mjs#unknownTools\)/,
  );
  const declared = fromJsonString(DeploymentDeclarationsSchema, dry.stdout);
  assert.deepEqual(
    declared.prompts.map((p) => [p.name, p.format, p.config, p.hash, p.origin]),
    [
      // Exports are visited in name order; the object behind two exports is kept once.
      [
        "fake/instructions",
        PromptFormat.PLAIN,
        "{}",
        promptHash("Be plain.", {}, "{}"),
        "src/index.mjs#agent.instructions",
      ],
      [
        "respond",
        PromptFormat.MUSTACHE,
        '{"temperature":0}',
        promptHash("Hi {{name}}", {}, '{"temperature":0}'),
        "src/index.mjs#respond",
      ],
    ],
  );
  const files = (name) =>
    declared.skills
      .find((s) => s.name === name)
      .files.map((f) => [f.path, f.executable, f.body.case, f.body.value]);
  assert.deepEqual(files("greet"), [
    ["SKILL.md", false, "content", "---\nname: greet\ndescription: Greets\n---\n\nHi.\n"],
    ["logo.png", false, "data", png],
    ["scripts/run.sh", true, "content", "#!/bin/sh\necho hi\n"],
  ]);
  assert.deepEqual(files("faq"), [
    ["SKILL.md", false, "content", "---\nname: faq\ndescription: FAQ\n---\n\nAnswer.\n"],
  ]);

  assert.deepEqual(
    declared.tools.map((t) => [
      t.name,
      t.description,
      t.summary,
      t.display,
      t.inputSchemaJson,
      t.outputSchemaJson,
      t.origin,
    ]),
    [
      [
        "ping",
        "Ping",
        "Pinged",
        ToolDisplay.HIDDEN,
        '{"type":"object"}',
        '{"type":"string"}',
        "src/index.mjs#again.tools.ping",
      ],
    ],
  );

  const clash = await run(dir, ["src/index.mjs"], { CLASH: "1" });
  assert.equal(clash.code, 1);
  assert.match(clash.stderr, /Prompt respond is declared twice with different content/);
  const toolClash = await run(dir, ["src/index.mjs"], { TOOL_CLASH: "1" });
  assert.equal(toolClash.code, 1);
  assert.match(
    toolClash.stderr,
    /Tool ping is declared twice with different definitions \(src\/index\.mjs#again\.tools\.ping, src\/index\.mjs#toolClash\.tools\.ping\)/,
  );
});
