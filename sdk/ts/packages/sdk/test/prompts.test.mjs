import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http2";
import { once } from "node:events";
import { create } from "@bufbuild/protobuf";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import {
  AgentContext,
  PromptFormat,
  canonicalJson,
  definePrompt,
  inference,
  promptHash,
  runWithContext,
} from "../dist/index.js";
import { InvokeRequestSchema } from "@trytilde/contracts/tilde/agent_host/v1/agent_pb.js";
import { SkillService } from "@trytilde/contracts/tilde/runtime/v1/skills_pb.js";

test("the prompt hash matches the engine's rule", () => {
  // Mirrors `hash_is_shared_with_the_sdk` in crates/tilde/src/prompts/mod.rs.
  assert.equal(
    promptHash(
      "Hello {{name}}\n{{> rules}}",
      { rules: "Be brief." },
      canonicalJson({ temperature: 0.2, model: "claude-sonnet-5" }),
    ),
    "985e00ff1447c634a4fbe5dca489e677fbdf175148e3a9933e05c884ebf9e691",
  );
});

test("module prompts and inference resolve the running invocation and stamp its calls", async () => {
  const server = createServer(
    connectNodeAdapter({
      routes(router) {
        router.service(SkillService, {
          async listSkills() {
            return {
              skills: [{ name: "refunds", description: "Handle refunds", paths: ["SKILL.md"] }],
            };
          },
          async readSkillFile(request) {
            return { content: `# ${request.name}/${request.path}`, mediaType: "text/markdown" };
          },
        });
      },
    }),
  );
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const port = server.address().port;
  const controller = new AbortController();
  const request = create(InvokeRequestSchema, {
    invocationId: "inv",
    runId: "run",
    threadId: "thread",
    agentId: "agent-1",
    objective: "Reply",
    callbackUrl: `http://127.0.0.1:${port}`,
    capability: "token-1",
  });
  const ctx = new AgentContext(request, controller, () => {});
  // Module scope: nothing here knows the invocation yet.
  const triage = definePrompt("triage", {
    template: "You help {{user}}.\n{{> tone}}",
    sections: { tone: "Be warm to {{user}}." },
    config: { model: "gpt-5", temperature: 0 },
  });
  const first = definePrompt("first", { template: "First" });
  const other = definePrompt("other", { template: "Other" });
  const model = inference("openai/prod");
  assert.deepEqual([...triage.variables], ["user"]);
  assert.equal(triage.format, PromptFormat.MUSTACHE);
  assert.equal(first.format, PromptFormat.PLAIN);
  assert.equal(
    definePrompt("triage", {
      template: "You help {{user}}.\n{{> tone}}",
      sections: { tone: "Be warm to {{user}}." },
      config: { temperature: 0, model: "gpt-5" },
    }).hash,
    triage.hash,
  );
  assert.throws(() => definePrompt("broken", { template: "{{> missing}}" }), /undeclared section/);
  assert.equal(triage.render({ user: "Ada" }), "You help Ada.\nBe warm to Ada.");
  await assert.rejects(model.fetch(`${model.baseURL}/responses`, {}), /outside a Tilde invocation/);

  const seen = [];
  const original = globalThis.fetch;
  globalThis.fetch = async (input, init) => {
    seen.push([new Request(input).url, new Headers(init.headers).get("x-tilde-prompt")]);
    return new Response("{}");
  };
  try {
    await runWithContext(ctx, async () => {
      await model.fetch(`${model.baseURL}/responses`, { method: "POST" });
      first.render();
      // A failed render must not become a stamp.
      assert.throws(() => triage.render({}), /user was not supplied/);
      await model.fetch(`${model.baseURL}/responses`, { method: "POST" });
      triage.render({ user: "Ada" });
      await model.fetch(`${model.baseURL}/responses`, { method: "POST" });
      await ctx.inference("openai/prod", { prompt: null }).fetch("http://x/", {});
      await ctx.inference("fast", { prompt: other }).fetch("http://x/", {});
    });
    // Outside `runWithContext`, `ctx.prompt` still binds renders to its invocation.
    ctx.prompt(other).render();
    await ctx.inference("openai/prod").fetch("http://x/", {});
  } finally {
    globalThis.fetch = original;
  }
  const routed = `http://127.0.0.1:${port}/inference/openai/prod/responses`;
  assert.deepEqual(seen, [
    [routed, null],
    [routed, first.stamp],
    [routed, `${first.stamp}, ${triage.stamp}`],
    ["http://x/", null],
    ["http://x/", other.stamp],
    ["http://x/", `${first.stamp}, ${triage.stamp}, ${other.stamp}`],
  ]);
  assert.equal(triage.telemetry["tilde.prompt.hash"], triage.hash);
  try {
    assert.deepEqual(
      (await ctx.skills.list()).map((s) => [s.name, s.description]),
      [["refunds", "Handle refunds"]],
    );
    assert.equal((await ctx.skills.read("refunds")).content, "# refunds/SKILL.md");
    assert.match(await ctx.skills.summary(), /- refunds: Handle refunds/);
  } finally {
    controller.abort();
    server.close();
  }
});
