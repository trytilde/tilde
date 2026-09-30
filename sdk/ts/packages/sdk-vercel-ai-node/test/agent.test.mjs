import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { ToolLoopAgent, tool, jsonSchema } from "ai";
import { MockLanguageModelV3 } from "ai/test";
import { PromptFormat, defineTools } from "@trytilde/sdk";
import { discover, discoverProject, tildeAiSdk, tildeCallOptions } from "../dist/index.js";

const usage = {
  inputTokens: { total: 1, noCache: 1, cacheRead: undefined, cacheWrite: undefined },
  outputTokens: { total: 1, text: 1, reasoning: undefined },
};
const call = (id) => ({
  content: [{ type: "tool-call", toolCallId: id, toolName: "sendMessage", input: '{"text":"hi"}' }],
  finishReason: { unified: "tool-calls", raw: undefined },
  usage,
  warnings: [],
});
const done = {
  content: [{ type: "text", text: "private" }],
  finishReason: { unified: "stop", raw: undefined },
  usage,
  warnings: [],
};

test("a ToolLoopAgent call carries channel tools, skills and steering that stays in the history", async () => {
  const steering = [];
  const sent = [];
  const ctx = {
    signal: new AbortController().signal,
    takeInputs: () => steering.splice(0),
    channel: {
      current: {
        sendMessage: {
          description: "Send",
          inputSchema: { type: "object", properties: { text: { type: "string" } } },
          execute: async (input, execution) => sent.push(execution.toolCallId),
        },
      },
    },
    setBundledTools: async () => {},
    skills: {
      summary: async () => "You have these skills.\n- refunds: Handle refunds",
      tools: () => ({
        read_skill: {
          description: "Read",
          inputSchema: { type: "object" },
          execute: async () => "",
        },
      }),
    },
  };
  const responses = [call("c1"), call("c2"), done];
  const model = new MockLanguageModelV3({
    doGenerate: async () => {
      // Steering sent while the first tool runs.
      if (model.doGenerateCalls.length === 1) steering.push({ id: "i1", text: "Also X" });
      return responses.shift();
    },
  });
  const agent = new ToolLoopAgent({
    model,
    instructions: "Be brief.",
    tools: {
      own: tool({ description: "Own", inputSchema: jsonSchema({ type: "object" }) }),
    },
    ...tildeCallOptions,
  });
  const result = await agent.generate({ prompt: "Hi", ...tildeAiSdk(ctx) });
  assert.equal(result.steps.length, 3);
  assert.deepEqual(sent, ["c1", "c2"]);
  const [first, second, third] = model.doGenerateCalls;
  assert.deepEqual(
    first.tools.map((t) => t.name),
    ["own", "sendMessage", "read_skill"],
  );
  assert.deepEqual(first.prompt[0], {
    role: "system",
    content: "Be brief.\n\nYou have these skills.\n- refunds: Handle refunds",
  });
  const roles = (prompt) =>
    prompt.map((m) => (m.role === "user" ? `user:${m.content[0].text}` : m.role));
  assert.deepEqual(roles(second.prompt), ["system", "user:Hi", "assistant", "tool", "user:Also X"]);
  // The next step is rebuilt from the call and responses; the input stays where it arrived.
  assert.deepEqual(roles(third.prompt), [
    "system",
    "user:Hi",
    "assistant",
    "tool",
    "user:Also X",
    "assistant",
    "tool",
  ]);
});

test("discover reads ToolLoopAgent instructions and eve's agent files", async () => {
  const origin = "dist/index.js#agent";
  const model = new MockLanguageModelV3();
  assert.deepEqual(
    (
      await discover(new ToolLoopAgent({ id: "support bot", model, instructions: "Be brief." }), {
        origin,
        projectDir: "/",
      })
    ).prompts,
    [
      {
        name: "support-bot/instructions",
        format: PromptFormat.PLAIN,
        template: "Be brief.",
        origin: `${origin}.instructions`,
      },
    ],
  );
  // Without an id the export names the prompt; each system message is sent (and matched) alone.
  assert.deepEqual(
    (
      await discover(
        new ToolLoopAgent({
          model,
          instructions: [
            { role: "system", content: "One." },
            { role: "system", content: "Two." },
          ],
        }),
        { origin, projectDir: "/" },
      )
    ).prompts.map((p) => [p.name, p.template, p.origin]),
    [
      ["agent/instructions/0", "One.", `${origin}.instructions[0]`],
      ["agent/instructions/1", "Two.", `${origin}.instructions[1]`],
    ],
  );
  assert.equal(await discover({ version: "agent-v1" }, { origin, projectDir: "/" }), undefined);

  const project = await mkdtemp(join(tmpdir(), "tilde-eve-"));
  assert.equal(await discoverProject(project), undefined);
  await mkdir(join(project, "agent", "skills", "triage"), { recursive: true });
  await writeFile(
    join(project, "agent", "skills", "triage", "SKILL.md"),
    "---\nname: triage\n---\n",
  );
  await writeFile(
    join(project, "agent", "instructions.mjs"),
    "export default ({ user }) => `Help ${user}.`;\n",
  );
  const found = await discoverProject(project);
  assert.deepEqual(found.prompts, [
    {
      name: "agent/instructions",
      format: PromptFormat.DYNAMIC,
      template: "({ user }) => `Help ${user}.`",
      origin: "agent/instructions.mjs#default",
    },
  ]);
  assert.deepEqual(found.skills, [
    { dir: join(project, "agent", "skills"), origin: "agent/skills" },
  ]);
  // Markdown instructions win, as eve's always-on system prompt.
  await writeFile(join(project, "agent", "instructions.md"), "You are concise.\n");
  assert.deepEqual(
    (await discoverProject(project)).prompts.map((p) => [p.name, p.format, p.template, p.origin]),
    [["agent/instructions", PromptFormat.PLAIN, "You are concise.\n", "agent/instructions.md"]],
  );
});

test("discover declares bundled tools and a ToolLoopAgent's tools as withTildeTools publishes them", async () => {
  const readOnly = { readOnly: true, destructive: false, idempotent: true, openWorld: false };
  const input = { type: "object", properties: { city: { type: "string" } } };
  const tools = {
    weather: tool({
      description: "Weather",
      inputSchema: jsonSchema(input),
      outputSchema: jsonSchema({ type: "object" }),
      metadata: { tilde: { summary: "Checked the weather", annotations: readOnly } },
      execute: async () => ({}),
    }),
    // Run by the provider or the caller, not this process: not declared.
    client: tool({ description: "Client", inputSchema: jsonSchema({ type: "object" }) }),
  };
  const at = (origin) => ({ origin, projectDir: "/" });
  const bundled = await discover(
    defineTools(tools, { weather: { display: "summary" } }),
    at("dist/index.js#bundledTools"),
  );
  assert.deepEqual(bundled.tools, [
    {
      name: "weather",
      description: "Weather",
      summary: "Checked the weather",
      annotations: readOnly,
      display: "summary",
      inputSchema: input,
      outputSchema: { type: "object" },
      origin: "dist/index.js#bundledTools.tools.weather",
    },
  ]);
  const agent = new ToolLoopAgent({ model: new MockLanguageModelV3(), instructions: "Hi.", tools });
  assert.deepEqual(
    (await discover(agent, at("dist/index.js#agent"))).tools.map((t) => [
      t.name,
      t.display,
      t.origin,
    ]),
    [["weather", undefined, "dist/index.js#agent.tools.weather"]],
  );
  // Another framework's tools are left to its adapter.
  assert.equal(await discover(defineTools([tools.weather]), at("x")), undefined);
});
