import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { Agent, handoff, shellTool, tool, webSearchTool } from "@openai/agents";
import { PromptFormat, defineTools, promptHash } from "@trytilde/sdk";
import { discover, tildeOpenAIAgents } from "../dist/index.js";

test("discover follows handoffs and agent tools without running instructions", async () => {
  const dir = await mkdtemp(join(tmpdir(), "tilde-openai-agents-"));
  await mkdir(join(dir, "triage"), { recursive: true });
  await writeFile(join(dir, "triage", "SKILL.md"), "---\nname: triage\ndescription: x\n---\n");
  let called = false;
  const instructions = () => {
    called = true;
    return "Dynamic";
  };
  const billing = new Agent({
    name: "Billing Desk",
    instructions,
    handoffDescription: "Handles invoices and refunds.",
  });
  const researcher = new Agent({ name: "researcher", instructions: "Research carefully." });
  const shell = shellTool({
    shell: { run: async () => ({ output: [] }) },
    environment: {
      type: "local",
      skills: [{ name: "triage", description: "x", path: join(dir, "triage") }],
    },
  });
  const support = new Agent({
    name: "support",
    instructions: "Help the customer.",
    handoffs: [handoff(billing)],
    tools: [researcher.asTool({ toolName: "research", toolDescription: "Research" }), shell],
    prompt: { promptId: "pmpt_1" },
  });
  // A cycle: billing can hand back to support.
  billing.handoffs = [support];

  const found = await discover(support, { origin: "src/index.ts#agent", projectDir: dir });
  assert.equal(called, false, "dynamic instructions are read as source, never called");
  assert.deepEqual(found.prompts, [
    {
      name: "support/instructions",
      format: PromptFormat.PLAIN,
      template: "Help the customer.",
      origin: "src/index.ts#agent.instructions",
    },
    {
      name: "Billing-Desk/instructions",
      format: PromptFormat.DYNAMIC,
      template: instructions.toString(),
      origin: "src/index.ts#agent.handoffs[0].instructions",
    },
    {
      name: "Billing-Desk/handoff_description",
      format: PromptFormat.PLAIN,
      template: "Handles invoices and refunds.",
      origin: "src/index.ts#agent.handoffs[0].handoffDescription",
    },
    {
      name: "researcher/instructions",
      format: PromptFormat.PLAIN,
      template: "Research carefully.",
      origin: "src/index.ts#agent.tools[0].instructions",
    },
  ]);
  assert.deepEqual(found.skills, [
    { dir: join(dir, "triage"), origin: "src/index.ts#agent.tools[1].environment.skills" },
  ]);
  assert.deepEqual(found.warnings, [
    "OpenAI agent support: its hosted prompt (prompt: { id }) is versioned by OpenAI, not Tilde (src/index.ts#agent.prompt)",
  ]);
  assert.equal(await discover({ name: "x" }, { origin: "x", projectDir: dir }), undefined);

  // At run time: registry skills join the shell tool's local skills, handoffs are cloned with the
  // channel tools, and the filter stamps the running agent's dynamic instructions and keeps
  // steering in place across turns.
  const stamps = [];
  const steering = [{ id: "s1", text: "Hurry" }];
  const ctx = {
    channel: {
      current: {
        sendMessage: {
          description: "Send",
          inputSchema: { type: "object" },
          execute: async () => ({}),
        },
      },
    },
    signal: new AbortController().signal,
    activatePrompt: (stamp) => stamps.push(stamp),
    takeInputs: () => steering.splice(0),
    setBundledTools: async () => {},
    skills: {
      list: async () => [
        { name: "faq", source: "team", description: "Answer FAQs", deployed: false },
        { name: "triage", source: "code", description: "x", deployed: true },
      ],
      directory: async () => "/cache/abc",
      summary: async () => "You have these skills.\n- faq: Answer FAQs",
      tools: () => ({
        read_skill: {
          description: "Read",
          inputSchema: { type: "object" },
          execute: async () => ({}),
        },
      }),
    },
  };
  const tilde = await tildeOpenAIAgents(ctx, support);
  assert.notEqual(tilde.agent, support);
  assert.equal(support.tools.length, 2, "the module-scope agent is left unchanged");
  assert.deepEqual(
    tilde.agent.tools.map((tool) => tool.name),
    ["research", "shell", "sendMessage"],
  );
  assert.deepEqual(tilde.agent.tools[1].environment.skills.at(-1), {
    name: "faq",
    description: "Answer FAQs",
    path: "/cache/abc/faq",
  });
  const billingCopy = tilde.agent.handoffs[0].agent;
  assert.equal(billingCopy.handoffs[0], tilde.agent, "cycles map onto the clones");
  // Billing has no shell tool: Tilde's skill tools and the summary instead.
  assert.deepEqual(
    billingCopy.tools.map((tool) => tool.name),
    ["sendMessage", "read_skill"],
  );
  assert.equal(
    await billingCopy.instructions(),
    "Dynamic\n\nYou have these skills.\n- faq: Answer FAQs",
  );

  const user = (text) => ({ role: "user", content: text });
  const first = await tilde.options.callModelInputFilter({
    modelData: { input: [user("Hi")], instructions: "x" },
    agent: billingCopy,
  });
  assert.deepEqual(first.input, [user("Hi"), user("Hurry")]);
  const later = await tilde.options.callModelInputFilter({
    modelData: { input: [user("Hi"), user("tool output")], instructions: "x" },
    agent: tilde.agent,
  });
  assert.deepEqual(later.input, [user("Hi"), user("Hurry"), user("tool output")]);
  assert.deepEqual(stamps, [
    { name: "Billing-Desk/instructions", hash: promptHash(instructions.toString(), {}, "{}") },
  ]);
});

test("discover declares bundled tools and an agent's function tools as withTildeTools publishes them", async () => {
  const readOnly = { readOnly: true, destructive: false, idempotent: true, openWorld: false };
  const input = {
    type: "object",
    properties: { city: { type: "string" } },
    required: ["city"],
    additionalProperties: false,
  };
  const weather = tool({
    name: "weather",
    description: "Weather",
    parameters: input,
    execute: async () => "Sunny",
  });
  const at = (origin) => ({ origin, projectDir: "/" });
  // Hosted tools run at OpenAI and are not declared.
  const tools = [weather, webSearchTool()];
  const found = await discover(
    defineTools(tools, {
      weather: { summary: "Checked the weather", display: "summary", annotations: readOnly },
    }),
    at("dist/index.js#bundledTools"),
  );
  assert.deepEqual(found, {
    tools: [
      {
        name: "weather",
        description: "Weather",
        summary: "Checked the weather",
        display: "summary",
        annotations: readOnly,
        inputSchema: input,
        outputSchema: undefined,
        origin: "dist/index.js#bundledTools.tools.weather",
      },
    ],
  });
  // Handoff agents' tools are theirs; only the exported agent's own are declared.
  const helper = new Agent({ name: "helper", instructions: "Help.", tools: [weather] });
  const agent = new Agent({ name: "support", instructions: "Help.", tools, handoffs: [helper] });
  assert.deepEqual(
    (await discover(agent, at("dist/index.js#agent"))).tools.map((t) => [t.name, t.origin]),
    [["weather", "dist/index.js#agent.tools.weather"]],
  );
});
