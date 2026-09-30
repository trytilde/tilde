import test from "node:test";
import assert from "node:assert/strict";
import { createServer as createHttp2Server } from "node:http2";
import { createServer } from "node:http";
import { once } from "node:events";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { create } from "@bufbuild/protobuf";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import { AgentContext } from "../dist/index.js";
import { InvokeRequestSchema } from "@trytilde/contracts/tilde/agent_host/v1/agent_pb.js";
import { SkillService } from "@trytilde/contracts/tilde/runtime/v1/skills_pb.js";

test("registry skills are kept per agent and updated by version, binaries downloaded", async (t) => {
  process.env.TMPDIR = mkdtempSync(join(tmpdir(), "tilde-skills-test-"));
  const logo = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0xff, 0x00]);
  const assets = createServer((_, res) => res.end(logo));
  assets.listen(0, "127.0.0.1");
  await once(assets, "listening");
  const skill = (name, source, versionId, deployed, paths) => ({
    name,
    source,
    versionId,
    deployed,
    description: name,
    files: paths.map((path) => ({ path, executable: path.endsWith(".sh") })),
  });
  let skills = [
    skill("shipped", "code", "v-shipped", true, ["SKILL.md"]),
    skill("refunds", "support", "v-refunds-1", false, ["SKILL.md", "logo.png", "run.sh"]),
    // Two sources share a name: folders are qualified by source.
    skill("style", "brand", "v-style-a", false, ["SKILL.md"]),
    skill("style", "docs", "v-style-b", false, ["SKILL.md"]),
  ];
  const reads = [];
  let lists = 0;
  const server = createHttp2Server(
    connectNodeAdapter({
      routes(router) {
        router.service(SkillService, {
          listSkills: async () => {
            lists += 1;
            return { skills };
          },
          async readSkillFile({ name, path }) {
            reads.push(`${name}/${path}`);
            return path.endsWith(".png")
              ? {
                  downloadUrl: `http://127.0.0.1:${assets.address().port}/logo`,
                  mediaType: "image/png",
                }
              : { content: `# ${name}`, mediaType: "text/markdown" };
          },
        });
      },
    }),
  );
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const controller = new AbortController();
  t.after(() => {
    controller.abort();
    server.close();
    assets.close();
    rmSync(process.env.TMPDIR, { recursive: true, force: true });
  });
  const ctx = new AgentContext(
    create(InvokeRequestSchema, {
      invocationId: "inv",
      runId: "run",
      threadId: "thread",
      agentId: "agent-1",
      callbackUrl: `http://127.0.0.1:${server.address().port}`,
      capability: "token-1",
    }),
    controller,
    () => {},
  );

  assert.deepEqual(
    (await ctx.skills.list()).map((s) => [s.name, s.deployed]),
    [
      ["shipped", true],
      ["refunds", false],
      ["style", false],
      ["style", false],
    ],
  );
  const root = await ctx.skills.directory();
  assert.equal(root, join(process.env.TMPDIR, "tilde-skills", "agent-1"));
  assert.deepEqual(
    readdirSync(root)
      .filter((f) => !f.startsWith("."))
      .sort(),
    ["brand-style", "docs-style", "refunds"],
  );
  assert.deepEqual(new Uint8Array(readFileSync(join(root, "refunds", "logo.png"))), logo);
  assert.equal(readFileSync(join(root, "docs-style", "SKILL.md"), "utf8"), "# docs/style");
  // Scripts keep their execute bit; other files get the default mode.
  assert.equal(statSync(join(root, "refunds", "run.sh")).mode & 0o777, 0o755);
  assert.equal(statSync(join(root, "refunds", "SKILL.md")).mode & 0o111, 0);
  assert(!reads.some((read) => read.startsWith("shipped")), "deployed skills are not fetched");

  // Same versions: nothing is read, from Tilde or from disk.
  let before = reads.length;
  assert.equal(await ctx.skills.directory(), root);
  assert.equal(reads.length, before);

  // A new version of one skill fetches only that skill; a removed skill's folder goes. The
  // remaining style no longer shares its name, so it moves to its plain folder.
  skills = skills
    .map((s) => (s.name === "refunds" ? { ...s, versionId: "v-refunds-2" } : s))
    .filter((s) => s.source !== "docs");
  before = reads.length;
  assert.equal(await ctx.skills.directory(), root);
  assert.deepEqual(
    reads.slice(before).sort((a, b) => a.localeCompare(b)),
    ["refunds/logo.png", "refunds/run.sh", "refunds/SKILL.md", "style/SKILL.md"],
  );
  assert.deepEqual(
    readdirSync(root)
      .filter((f) => !f.startsWith("."))
      .sort(),
    ["refunds", "style"],
  );
  assert.deepEqual(JSON.parse(readFileSync(join(root, ".versions.json"), "utf8")), {
    refunds: "v-refunds-2",
    style: "v-style-a",
  });

  // Skills pushed with the wake answer without a list call, and update the same folder.
  const pushed = new AgentContext(
    create(InvokeRequestSchema, {
      invocationId: "inv-2",
      runId: "run",
      threadId: "thread",
      agentId: "agent-1",
      callbackUrl: `http://127.0.0.1:${server.address().port}`,
      capability: "token-1",
      state: { skills: skills.filter((s) => s.name !== "style") },
    }),
    controller,
    () => {},
  );
  const listed = lists;
  assert.deepEqual(
    (await pushed.skills.list()).map((s) => s.name),
    ["shipped", "refunds"],
  );
  assert.equal(await pushed.skills.directory(), root);
  assert.equal(lists, listed);
  assert.deepEqual(
    readdirSync(root).filter((f) => !f.startsWith(".")),
    ["refunds"],
  );

  // Two invocations syncing at once download each change once: refunds' new version and style,
  // which this invocation has but the last one did not (twice over would be 8).
  skills = skills.map((s) => (s.name === "refunds" ? { ...s, versionId: "v-refunds-3" } : s));
  before = reads.length;
  const [a, b] = await Promise.all([ctx.skills.directory(), ctx.skills.directory()]);
  assert.equal(a, root);
  assert.equal(b, root);
  assert.equal(reads.length - before, 4);

  // A new process (a restarted agent) reuses the folder through .versions.json: nothing is read.
  before = reads.length;
  const script = `
    import { create } from "@bufbuild/protobuf";
    import { AgentContext } from "./dist/index.js";
    import { InvokeRequestSchema } from "@trytilde/contracts/tilde/agent_host/v1/agent_pb.js";
    const ctx = new AgentContext(
      create(InvokeRequestSchema, {
        invocationId: "inv-3", runId: "run", threadId: "thread", agentId: "agent-1",
        callbackUrl: "http://127.0.0.1:${server.address().port}", capability: "token-1",
      }),
      new AbortController(),
      () => {},
    );
    process.stdout.write(await ctx.skills.directory());
    process.exit(0);
  `;
  const child = spawn(process.execPath, ["--input-type=module", "-e", script], {
    cwd: join(import.meta.dirname, ".."),
    env: process.env,
  });
  let printed = "";
  child.stdout.on("data", (chunk) => (printed += chunk));
  const [code] = await once(child, "exit");
  assert.equal(code, 0);
  assert.equal(printed, root);
  assert.equal(reads.length, before);
});
