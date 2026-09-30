import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { Agent } from "@mastra/core/agent";
import { Mastra } from "@mastra/core";
import { createSkill } from "@mastra/core/skills";
import { createTool } from "@mastra/core/tools";
import { tool as aiTool, jsonSchema } from "ai";
import { PromptFormat, defineTools, promptHash } from "@trytilde/sdk";
import { discover, tildeMastra } from "../dist/index.js";

test("discover reads what Mastra agents declare without running them", async () => {
  const dir = await mkdtemp(join(tmpdir(), "tilde-mastra-"));
  await mkdir(join(dir, "skills", "triage"), { recursive: true });
  await writeFile(
    join(dir, "skills", "triage", "SKILL.md"),
    "---\nname: triage\ndescription: Sort requests\n---\n\nSort them.\n",
  );
  let called = false;
  const instructions = () => {
    called = true;
    return "Dynamic";
  };
  const support = new Agent({
    id: "support",
    name: "Support",
    instructions,
    model: "openai/gpt-4o-mini",
    skills: [
      join(dir, "skills"),
      createSkill({
        name: "faq",
        description: "Answer common\nquestions",
        instructions: "Answer from the FAQ.",
        references: { "faq.md": "Q: A?" },
      }),
    ],
  });
  const plain = new Agent({
    id: "Plain Agent",
    name: "Plain",
    instructions: ["Be brief.", { role: "system", content: "Be kind." }],
    model: "openai/gpt-4o-mini",
  });
  const mastra = new Mastra({ agents: { support, plain } });

  const found = await discover(mastra, { origin: "src/index.ts#mastra", projectDir: dir });
  assert.equal(called, false, "dynamic instructions are read as source, never called");
  assert.deepEqual(found.prompts, [
    {
      name: "support/instructions",
      format: PromptFormat.DYNAMIC,
      template: instructions.toString(),
      origin: "src/index.ts#mastra.agents.support.instructions",
    },
    {
      name: "Plain-Agent/instructions",
      format: PromptFormat.PLAIN,
      template: "Be brief.\n\nBe kind.",
      origin: "src/index.ts#mastra.agents.plain.instructions",
    },
  ]);
  assert.deepEqual(found.skills, [
    { dir: join(dir, "skills", "triage"), origin: "src/index.ts#mastra.agents.support.skills" },
    {
      name: "faq",
      files: [
        {
          path: "SKILL.md",
          content:
            "---\nname: faq\ndescription: Answer common questions\n---\n\nAnswer from the FAQ.\n",
        },
        { path: "references/faq.md", content: "Q: A?" },
      ],
      origin: "src/index.ts#mastra.agents.support.skills",
    },
  ]);
  assert.equal(await discover({ id: "x" }, { origin: "x", projectDir: dir }), undefined);

  // At run time the processor stamps the dynamic instructions and hands over steering.
  const stamps = [];
  const steering = [{ id: "s1", text: "Hurry" }];
  const ctx = {
    channel: { current: {} },
    agentTools: {},
    setBundledTools: async () => {},
    signal: new AbortController().signal,
    activatePrompt: (stamp) => stamps.push(stamp),
    takeInputs: () => steering.splice(0),
  };
  const [processor] = (await tildeMastra(ctx)).inputProcessors;
  const added = [];
  await processor.processInputStep({
    agent: support,
    messageList: { add: (message, source) => added.push([message, source]) },
  });
  assert.deepEqual(stamps, [
    { name: "support/instructions", hash: promptHash(instructions.toString(), {}, "{}") },
  ]);
  assert.deepEqual(added, [[{ role: "user", content: "Hurry" }, "input"]]);
});

test("discover declares bundled tools and an agent's static tools as withTildeTools publishes them", async () => {
  const weather = createTool({
    id: "weather",
    description: "Weather",
    inputSchema: { type: "object", properties: { city: { type: "string" } } },
    mcp: {
      annotations: { readOnlyHint: true, idempotentHint: true },
      _meta: { tilde: { summary: "Checked the weather" } },
    },
    execute: async () => ({}),
  });
  // An AI SDK tool in the record passes through withTildeTools unpublished: not declared.
  const other = aiTool({ description: "Other", inputSchema: jsonSchema({ type: "object" }) });
  const at = (origin) => ({ origin, projectDir: "/" });
  const found = await discover(
    defineTools({ weather, other }, { weather: { display: "hidden" } }),
    at("dist/index.js#bundledTools"),
  );
  assert.equal(found.tools.length, 1);
  const [declared] = found.tools;
  assert.equal(declared.origin, "dist/index.js#bundledTools.tools.weather");
  assert.equal(declared.summary, "Checked the weather");
  assert.equal(declared.display, "hidden");
  assert.deepEqual(declared.annotations, {
    readOnly: true,
    destructive: true,
    idempotent: true,
    openWorld: true,
  });
  assert.deepEqual(declared.inputSchema.properties, { city: { type: "string" } });

  const agent = new Agent({
    id: "support",
    name: "Support",
    instructions: "Help.",
    model: "openai/gpt-4o-mini",
    tools: { weather },
  });
  const fromAgent = await discover(agent, at("dist/index.js#agent"));
  assert.deepEqual(
    fromAgent.tools.map((t) => [t.name, t.summary, t.display, t.origin]),
    [["weather", "Checked the weather", undefined, "dist/index.js#agent.tools.weather"]],
  );
  // Tools resolved per call cannot be declared.
  const dynamic = new Agent({
    id: "dynamic",
    name: "Dynamic",
    instructions: "Help.",
    model: "openai/gpt-4o-mini",
    tools: () => ({ weather }),
  });
  assert.deepEqual((await discover(dynamic, at("x"))).tools, []);
});
