import assert from "node:assert/strict";
import { test } from "node:test";
import { RunContext, tool } from "@openai/agents";
import { convertToOpenAIAgentsTools, withTildeTools } from "../dist/index.js";

test("tool conversion preserves schemas, overrides instructions and forwards execution IDs", async () => {
  const calls = [];
  const schema = { type: "object", properties: { value: { type: "number" } }, required: ["value"] };
  const source = {
    description: "Provider instructions",
    inputSchema: schema,
    execute: async (input, execution) => {
      calls.push({ input, execution });
      return { ok: true };
    },
  };
  const [converted] = convertToOpenAIAgentsTools(
    { customer_action: source },
    { prefix: "custom_", instructions: { customer_action: "Custom model instructions" } },
  );
  assert.equal(converted.type, "function");
  assert.equal(converted.name, "custom_customer_action");
  assert.equal(converted.description, "Custom model instructions");
  assert.equal(converted.strict, false);
  assert.deepEqual(converted.parameters, schema);
  // The runner invokes tools with the serialized arguments plus the model's function_call item.
  const details = (callId, args, signal) => ({
    toolCall: { type: "function_call", callId, name: converted.name, arguments: args },
    signal,
  });
  assert.deepEqual(
    await converted.invoke(
      new RunContext(),
      JSON.stringify({ value: 3 }),
      details("call-123", JSON.stringify({ value: 3 }), new AbortController().signal),
    ),
    { ok: true },
  );
  assert.deepEqual(
    calls.map(({ input, execution }) => [input, execution.toolCallId]),
    [[{ value: 3 }, "call-123"]],
  );
  const stop = new AbortController();
  stop.abort();
  await assert.rejects(
    converted.invoke(
      new RunContext(),
      JSON.stringify({ value: 4 }),
      details("canceled", JSON.stringify({ value: 4 }), stop.signal),
    ),
  );
  assert.equal(calls.length, 1);
  assert.throws(() => convertToOpenAIAgentsTools({ "a.b": source, "a-b": source }), /Conflicting/);
});

// A stand-in for AgentContext: it records what the adapter publishes and audits, and its
// tools.execute routes a bundled name to the published executor the way the core SDK does.
function context() {
  const reports = [];
  const published = [];
  const sendMessage = {
    description: "Send a message.",
    providerId: "native",
    inputSchema: { type: "object", properties: { text: { type: "string" } } },
    execute: async () => ({ sent: true }),
  };
  const toolsExecute = {
    description: "Call a tool found with tools.search.",
    inputSchema: { type: "object" },
    // Names without a published executor go to Tilde.
    execute: async ({ name, input }, execution) => {
      const bundled = published.find((tool) => tool.name === name);
      return bundled ? bundled.execute(input ?? {}, execution) : { routed: name };
    },
  };
  return {
    reports,
    published,
    signal: new AbortController().signal,
    channel: { current: { sendMessage } },
    agentTools: { "tools.execute": toolsExecute },
    reportToolCall: async (report) => void reports.push(report),
    registrations: 0,
    async setBundledTools(tools) {
      this.registrations++;
      published.splice(0, published.length, ...tools);
    },
  };
}
const invoke = (target, runContext, callId, args) =>
  target.invoke(runContext, JSON.stringify(args), {
    toolCall: { type: "function_call", callId, name: target.name, arguments: JSON.stringify(args) },
    signal: new AbortController().signal,
  });

void test("withTildeTools publishes, audits and routes the agent's own function tools", async () => {
  const ctx = context();
  const users = [];
  const parameters = {
    type: "object",
    properties: { sides: { type: "integer" } },
    required: ["sides"],
    additionalProperties: false,
  };
  const rollDice = tool({
    name: "roll_dice",
    description: "Roll a die.",
    parameters,
    strict: true,
    execute: async ({ sides }, runContext) => {
      users.push(runContext.context.user);
      return { roll: sides };
    },
  });
  const fail = tool({
    name: "fail",
    description: "Always fails.",
    parameters: { type: "object", properties: {}, required: [], additionalProperties: false },
    strict: true,
    // Without an errorFunction override the SDK answers the model with the error text.
    errorFunction: null,
    execute: async () => {
      throw new Error("broken");
    },
  });
  const tools = await withTildeTools(ctx, [rollDice, fail], {
    roll_dice: {
      summary: "Rolled a die",
      display: "summary",
      annotations: { readOnly: true, destructive: false, idempotent: false, openWorld: false },
    },
  });
  assert.deepEqual(
    tools.map((t) => t.name),
    ["sendMessage", "tools_execute", "roll_dice", "fail"],
  );
  assert.deepEqual(
    ctx.published.map((t) => [t.name, t.description, t.summary, t.display]),
    [
      ["roll_dice", "Roll a die.", "Rolled a die", "summary"],
      ["fail", "Always fails.", undefined, undefined],
    ],
  );
  assert.deepEqual(ctx.published[0].inputSchema, parameters);
  assert.equal(ctx.published[0].annotations.readOnly, true);
  const [, toolsExecute, roll, failing] = tools;

  const runContext = new RunContext({ user: "ada" });
  assert.deepEqual(await invoke(roll, runContext, "call-1", { sides: 6 }), { roll: 6 });
  await assert.rejects(invoke(failing, runContext, "call-2", {}), /broken/);
  // The model calls tools.execute naming the bundled tool: it runs here with the run's
  // RunContext, audited once.
  assert.deepEqual(
    await invoke(toolsExecute, runContext, "call-3", { name: "roll_dice", input: { sides: 4 } }),
    { roll: 4 },
  );
  assert.deepEqual(users, ["ada", "ada"]);
  assert.deepEqual(
    ctx.reports.map((r) => [r.toolCallId, r.name, r.status, r.summary, r.display]),
    [
      ["call-1", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-1", "roll_dice", "completed", "Rolled a die", "summary"],
      ["call-2", "fail", "running", undefined, undefined],
      ["call-2", "fail", "failed", undefined, undefined],
      ["call-3", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-3", "roll_dice", "completed", "Rolled a die", "summary"],
    ],
  );
  assert.deepEqual(ctx.reports[0].input, { sides: 6 });
  assert.deepEqual(ctx.reports[1].output, { roll: 6 });

  // With the default errorFunction the failure becomes the tool's string result, so it is
  // audited as completed with the error text.
  const caught = tool({
    name: "caught",
    description: "Fails into a string.",
    parameters: { type: "object", properties: {}, required: [], additionalProperties: false },
    execute: async () => {
      throw new Error("broken");
    },
  });
  const [, , wrapped] = await withTildeTools(ctx, [caught]);
  assert.match(await invoke(wrapped, runContext, "call-4", {}), /broken/);
  assert.deepEqual(
    ctx.reports.slice(-1).map((r) => [r.status, typeof r.output]),
    [["completed", "string"]],
  );
  await assert.rejects(
    withTildeTools(ctx, [
      tool({
        name: "sendMessage",
        description: "Clashes with the channel tool.",
        parameters: caught.parameters,
        execute: async () => "",
      }),
    ]),
    /conflicts with another tool/,
  );
});

void test("withTildeTools with no tools unpublishes the last bundled tool", async () => {
  const ctx = context();
  const runs = [];
  const oldTool = tool({
    name: "old_tool",
    description: "Removed later.",
    parameters: { type: "object", properties: {}, required: [], additionalProperties: false },
    strict: true,
    execute: async () => void runs.push("old_tool"),
  });
  await withTildeTools(ctx, [oldTool]);
  const tools = await withTildeTools(ctx, []);
  assert(!tools.map((t) => t.name).includes("old_tool"));
  // The empty set is registered, so a tools.execute naming the old tool no longer runs it here.
  assert.equal(ctx.registrations, 2);
  assert.deepEqual(ctx.published, []);
  assert.deepEqual(
    await ctx.agentTools["tools.execute"].execute(
      { name: "old_tool", input: {} },
      { toolCallId: "call-1" },
    ),
    { routed: "old_tool" },
  );
  assert.deepEqual(runs, []);
});

// The server allows a 64-character tool name after its source slug, so catalog names can be longer
// than the 64 characters models accept.
const STAGES = "customer_relationship_hub.list_open_opportunities_for_owner_by_stages";
const REGION = "customer_relationship_hub.list_open_opportunities_for_owner_by_region";
const catalogTool = (calls, name) => ({
  description: name,
  inputSchema: { type: "object" },
  execute: async () => void calls.push(name),
});

void test("long catalog names get stable, distinct model aliases bound to their tools", async () => {
  const calls = [];
  const tools = convertToOpenAIAgentsTools({
    [STAGES]: catalogTool(calls, STAGES),
    [REGION]: catalogTool(calls, REGION),
  });
  const [stages, region] = tools.map((t) => t.name);
  // The same alias in every adapter and in the Python SDK.
  assert.equal(stages, "customer_relationship_hub_list_open_opportunities_for_o_d9926af9");
  assert.equal(region.length, 64);
  assert.notEqual(stages, region);
  await tools[1].invoke(new RunContext(), "{}", {
    toolCall: { type: "function_call", callId: "call-1", name: region, arguments: "{}" },
  });
  assert.deepEqual(calls, [REGION]);
});
