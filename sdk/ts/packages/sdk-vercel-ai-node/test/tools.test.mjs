import assert from "node:assert/strict";
import { test } from "node:test";
import { jsonSchema, tool } from "ai";
import { convertToAiSdkTools, withTildeTools } from "../dist/index.js";

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
  const tools = convertToAiSdkTools(
    { customer_action: source },
    { prefix: "custom_", instructions: { customer_action: "Custom model instructions" } },
  );
  assert.equal(tools.custom_customer_action.description, "Custom model instructions");
  assert.deepEqual(await tools.custom_customer_action.inputSchema.jsonSchema, schema);
  assert.deepEqual(
    await tools.custom_customer_action.execute(
      { value: 3 },
      { toolCallId: "call-123", messages: [], abortSignal: new AbortController().signal },
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
    tools.custom_customer_action.execute(
      { value: 4 },
      { toolCallId: "canceled", messages: [], abortSignal: stop.signal },
    ),
  );
  assert.equal(calls.length, 1);
  assert.throws(() => convertToAiSdkTools({ "a.b": source, a_b: source }), /Conflicting/);
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
const options = (toolCallId) => ({
  toolCallId,
  messages: [],
  abortSignal: new AbortController().signal,
});

void test("withTildeTools publishes, audits and routes the agent's own AI SDK tools", async () => {
  const ctx = context();
  const rolls = [];
  const tools = await withTildeTools(
    ctx,
    {
      roll_dice: tool({
        description: "Roll a die.",
        inputSchema: jsonSchema(
          { type: "object", properties: { sides: { type: "integer" } }, required: ["sides"] },
          {
            validate: (value) =>
              Number.isInteger(value?.sides)
                ? { success: true, value }
                : { success: false, error: new Error("sides must be an integer") },
          },
        ),
        outputSchema: jsonSchema({ type: "object", properties: { roll: { type: "integer" } } }),
        metadata: { tilde: { summary: "Rolled a die", display: "summary" } },
        execute: async ({ sides }) => {
          rolls.push(sides);
          return { roll: sides };
        },
      }),
      fail: tool({
        description: "Always fails.",
        inputSchema: jsonSchema({ type: "object" }),
        execute: async () => {
          throw new Error("broken");
        },
      }),
      // Preliminary results stream from an AsyncIterable; only the final value is kept.
      count: tool({
        description: "Counts.",
        inputSchema: jsonSchema({ type: "object" }),
        async *execute() {
          yield { n: 1 };
          yield { n: 2 };
        },
      }),
    },
    { fail: { summary: "Failed on purpose", display: "hidden" } },
  );
  assert.deepEqual(Object.keys(tools).sort(), [
    "count",
    "fail",
    "roll_dice",
    "sendMessage",
    "tools_execute",
  ]);
  assert.deepEqual(
    ctx.published.map((t) => [t.name, t.description, t.summary, t.display]),
    [
      ["roll_dice", "Roll a die.", "Rolled a die", "summary"],
      ["fail", "Always fails.", "Failed on purpose", "hidden"],
      ["count", "Counts.", undefined, undefined],
    ],
  );
  assert.deepEqual(ctx.published[0].inputSchema.required, ["sides"]);
  assert.deepEqual(ctx.published[0].outputSchema.properties.roll, { type: "integer" });
  assert.equal(ctx.published[1].outputSchema, undefined);

  assert.deepEqual(await tools.roll_dice.execute({ sides: 6 }, options("call-1")), { roll: 6 });
  await assert.rejects(tools.fail.execute({}, options("call-2")), /broken/);
  assert.deepEqual(await tools.count.execute({}, options("call-3")), { n: 2 });
  // The model calls tools.execute naming the bundled tool: it runs here, audited once.
  assert.deepEqual(
    await tools.tools_execute.execute(
      { name: "roll_dice", input: { sides: 4 } },
      options("call-4"),
    ),
    { roll: 4 },
  );
  // Routed input is validated as the AI SDK would before the tool runs.
  await assert.rejects(
    tools.tools_execute.execute({ name: "roll_dice", input: {} }, options("call-5")),
    /sides must be an integer/,
  );
  assert.deepEqual(rolls, [6, 4]);
  assert.deepEqual(
    ctx.reports.map((r) => [r.toolCallId, r.name, r.status, r.summary, r.display]),
    [
      ["call-1", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-1", "roll_dice", "completed", "Rolled a die", "summary"],
      ["call-2", "fail", "running", "Failed on purpose", "hidden"],
      ["call-2", "fail", "failed", "Failed on purpose", "hidden"],
      ["call-3", "count", "running", undefined, undefined],
      ["call-3", "count", "completed", undefined, undefined],
      ["call-4", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-4", "roll_dice", "completed", "Rolled a die", "summary"],
    ],
  );
  assert.deepEqual(ctx.reports[1].output, { roll: 6 });
  assert.equal(ctx.reports[3].error, "broken");
  await assert.rejects(
    withTildeTools(ctx, { sendMessage: tools.roll_dice }),
    /conflicts with another tool/,
  );
});

void test("withTildeTools with no tools unpublishes the last bundled tool", async () => {
  const ctx = context();
  const runs = [];
  const oldTool = tool({
    description: "Removed later.",
    inputSchema: jsonSchema({ type: "object" }),
    execute: async () => void runs.push("old_tool"),
  });
  await withTildeTools(ctx, { old_tool: oldTool });
  const tools = await withTildeTools(ctx, {});
  assert(!Object.keys(tools).includes("old_tool"));
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
  const tools = convertToAiSdkTools({
    [STAGES]: catalogTool(calls, STAGES),
    [REGION]: catalogTool(calls, REGION),
  });
  const [stages, region] = Object.keys(tools);
  // The same alias in every adapter and in the Python SDK.
  assert.equal(stages, "customer_relationship_hub_list_open_opportunities_for_o_d9926af9");
  assert.equal(region.length, 64);
  assert.notEqual(stages, region);
  await tools[region].execute({}, options("call-1"));
  assert.deepEqual(calls, [REGION]);
});
