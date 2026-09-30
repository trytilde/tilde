import assert from "node:assert/strict";
import { test } from "node:test";
import { ToolMessage } from "@langchain/core/messages";
import { tool } from "@langchain/core/tools";
import { convertToLangChainTools, withTildeTools } from "../dist/index.js";

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
  const [converted] = convertToLangChainTools(
    { customer_action: source },
    { prefix: "custom_", instructions: { customer_action: "Custom model instructions" } },
  );
  assert.equal(converted.name, "custom_customer_action");
  assert.equal(converted.description, "Custom model instructions");
  assert.deepEqual(converted.schema, schema);
  // LangGraph's ToolNode and langchain's createAgent call tool.invoke with the model's ToolCall.
  const output = await converted.invoke(
    { id: "call-123", name: "custom_customer_action", args: { value: 3 }, type: "tool_call" },
    { signal: new AbortController().signal },
  );
  assert(output instanceof ToolMessage);
  assert.equal(output.tool_call_id, "call-123");
  assert.equal(output.content, JSON.stringify({ ok: true }));
  assert.deepEqual(
    calls.map(({ input, execution }) => [input, execution.toolCallId]),
    [[{ value: 3 }, "call-123"]],
  );
  const stop = new AbortController();
  stop.abort();
  await assert.rejects(
    converted.invoke(
      { id: "canceled", name: "custom_customer_action", args: { value: 4 }, type: "tool_call" },
      { signal: stop.signal },
    ),
  );
  assert.equal(calls.length, 1);
  assert.throws(() => convertToLangChainTools({ "a.b": source, a_b: source }), /Conflicting/);
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
const call = (id, name, args) => ({ type: "tool_call", id, name, args });

void test("withTildeTools publishes, audits and routes the agent's own LangChain tools", async () => {
  const ctx = context();
  const configs = [];
  const rollDice = tool(
    async ({ sides }, config) => {
      configs.push(config.configurable?.thread);
      return { roll: sides };
    },
    {
      name: "roll_dice",
      description: "Roll a die.",
      schema: { type: "object", properties: { sides: { type: "integer" } }, required: ["sides"] },
      metadata: { tilde: { summary: "Rolled a die", display: "summary" } },
    },
  );
  const fail = tool(
    async () => {
      throw new Error("broken");
    },
    { name: "fail", description: "Always fails.", schema: { type: "object" } },
  );
  const tools = await withTildeTools(ctx, [rollDice, fail], {
    fail: { summary: "Failed on purpose", display: "hidden" },
  });
  assert.deepEqual(
    tools.map((t) => t.name),
    ["sendMessage", "tools_execute", "roll_dice", "fail"],
  );
  assert.deepEqual(
    ctx.published.map((t) => [t.name, t.description, t.summary, t.display]),
    [
      ["roll_dice", "Roll a die.", "Rolled a die", "summary"],
      ["fail", "Always fails.", "Failed on purpose", "hidden"],
    ],
  );
  assert.deepEqual(ctx.published[0].inputSchema.required, ["sides"]);
  const [, toolsExecute, roll, failing] = tools;

  const config = { configurable: { thread: "t-1" } };
  const message = await roll.invoke(call("call-1", "roll_dice", { sides: 6 }), config);
  assert(message instanceof ToolMessage);
  assert.equal(message.tool_call_id, "call-1");
  await assert.rejects(failing.invoke(call("call-2", "fail", {})), /broken/);
  // Input the schema rejects never reaches the tool body but is still audited.
  await assert.rejects(roll.invoke(call("call-3", "roll_dice", { sides: "six" })));
  // The model calls tools.execute naming the bundled tool: it runs here with the graph's config,
  // audited once, and answers the outer call.
  const routed = await toolsExecute.invoke(
    call("call-4", "tools_execute", { name: "roll_dice", input: { sides: 4 } }),
    config,
  );
  assert.equal(routed.tool_call_id, "call-4");
  assert.deepEqual(JSON.parse(routed.content), { roll: 4 });
  assert.deepEqual(configs, ["t-1", "t-1"]);
  assert.deepEqual(
    ctx.reports.map((r) => [r.toolCallId, r.name, r.status, r.summary, r.display]),
    [
      ["call-1", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-1", "roll_dice", "completed", "Rolled a die", "summary"],
      ["call-2", "fail", "running", "Failed on purpose", "hidden"],
      ["call-2", "fail", "failed", "Failed on purpose", "hidden"],
      ["call-3", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-3", "roll_dice", "failed", "Rolled a die", "summary"],
      ["call-4", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-4", "roll_dice", "completed", "Rolled a die", "summary"],
    ],
  );
  assert.deepEqual(ctx.reports[0].input, { sides: 6 });
  assert.equal(ctx.reports[1].output, JSON.stringify({ roll: 6 }));
  assert.deepEqual(ctx.reports[7].output, { roll: 4 });
  await assert.rejects(
    withTildeTools(ctx, [tool(async () => "", { name: "tools_execute", schema: {} })]),
    /conflicts with another tool/,
  );
});

void test("withTildeTools with no tools unpublishes the last bundled tool", async () => {
  const ctx = context();
  const runs = [];
  const oldTool = tool(async () => void runs.push("old_tool"), {
    name: "old_tool",
    description: "Removed later.",
    schema: { type: "object", properties: {} },
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
  const tools = convertToLangChainTools({
    [STAGES]: catalogTool(calls, STAGES),
    [REGION]: catalogTool(calls, REGION),
  });
  const [stages, region] = tools.map((t) => t.name);
  // The same alias in every adapter and in the Python SDK.
  assert.equal(stages, "customer_relationship_hub_list_open_opportunities_for_o_d9926af9");
  assert.equal(region.length, 64);
  assert.notEqual(stages, region);
  await tools[1].invoke(call("call-1", region, {}));
  assert.deepEqual(calls, [REGION]);
});
