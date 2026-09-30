import assert from "node:assert/strict";
import { test } from "node:test";
import { RequestContext } from "@mastra/core/request-context";
import { createTool, isValidationError } from "@mastra/core/tools";
import { convertToMastraTools, withTildeTools } from "../dist/index.js";

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
  const tools = convertToMastraTools(
    { customer_action: source },
    { prefix: "custom_", instructions: { customer_action: "Custom model instructions" } },
  );
  const tool = tools.custom_customer_action;
  assert.equal(tool.id, "custom_customer_action");
  assert.equal(tool.description, "Custom model instructions");
  // Mastra adds only a draft-07 `$schema` marker to the published provider schema.
  const published = tool.inputSchema["~standard"].jsonSchema.input({ target: "draft-07" });
  delete published.$schema;
  assert.deepEqual(published, schema);
  // Mastra runs the agent loop on the AI SDK and passes its tool-call options to execute.
  const execution = {
    toolCallId: "call-123",
    messages: [],
    abortSignal: new AbortController().signal,
  };
  assert.deepEqual(await tool.execute({ value: 3 }, execution), { ok: true });
  assert.deepEqual(
    calls.map(({ input, execution }) => [input, execution.toolCallId]),
    [[{ value: 3 }, "call-123"]],
  );
  // The preserved schema is enforced before the channel is reached.
  assert(isValidationError(await tool.execute({}, execution)));
  const stop = new AbortController();
  stop.abort();
  await assert.rejects(
    tool.execute({ value: 4 }, { ...execution, toolCallId: "canceled", abortSignal: stop.signal }),
  );
  assert.equal(calls.length, 1);
  assert.throws(() => convertToMastraTools({ "a.b": source, a_b: source }), /Conflicting/);
});

test("a direct execute outside an agent run still gets a unique execution ID", async () => {
  const ids = [];
  const tools = convertToMastraTools({
    ping: {
      description: "Ping",
      inputSchema: { type: "object" },
      execute: async (_input, execution) => ids.push(execution.toolCallId),
    },
  });
  await tools.ping.execute({});
  await tools.ping.execute({});
  assert.equal(new Set(ids).size, 2);
  assert(ids.every((id) => /^[0-9a-f-]{36}$/.test(id)));
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

void test("withTildeTools publishes, audits and routes the agent's own Mastra tools", async () => {
  const ctx = context();
  const users = [];
  const rollDice = createTool({
    id: "roll-dice",
    description: "Roll a die.",
    inputSchema: {
      type: "object",
      properties: { sides: { type: "integer" } },
      required: ["sides"],
    },
    outputSchema: { type: "object", properties: { roll: { type: "integer" } } },
    mcp: { annotations: { readOnlyHint: true }, _meta: { tilde: { summary: "Rolled a die" } } },
    execute: async ({ sides }, { requestContext }) => {
      users.push(requestContext?.get("user"));
      return { roll: sides };
    },
  });
  const fail = createTool({
    id: "fail",
    description: "Always fails.",
    inputSchema: { type: "object" },
    execute: async () => {
      throw new Error("broken");
    },
  });
  const tools = await withTildeTools(
    ctx,
    { roll_dice: rollDice, fail },
    { roll_dice: { display: "summary" }, fail: { summary: "Failed on purpose" } },
  );
  assert.deepEqual(Object.keys(tools), ["sendMessage", "tools_execute", "roll_dice", "fail"]);
  // The published name is the record key the model sees, not the tool's id.
  assert.deepEqual(
    ctx.published.map((t) => [t.name, t.description, t.summary, t.display]),
    [
      ["roll_dice", "Roll a die.", "Rolled a die", "summary"],
      ["fail", "Always fails.", "Failed on purpose", undefined],
    ],
  );
  assert.deepEqual(ctx.published[0].inputSchema.required, ["sides"]);
  assert.deepEqual(ctx.published[0].outputSchema.properties.roll, { type: "integer" });
  assert.deepEqual(ctx.published[0].annotations, {
    readOnly: true,
    destructive: true,
    idempotent: false,
    openWorld: true,
  });
  assert.equal(ctx.published[1].annotations, undefined);

  const requestContext = new RequestContext([["user", "ada"]]);
  const execution = (toolCallId) => ({
    toolCallId,
    messages: [],
    abortSignal: new AbortController().signal,
    requestContext,
  });
  assert(tools.roll_dice instanceof rollDice.constructor);
  assert.deepEqual(await tools.roll_dice.execute({ sides: 6 }, execution("call-1")), { roll: 6 });
  await assert.rejects(tools.fail.execute({}, execution("call-2")), /broken/);
  // Mastra returns a ValidationError for invalid input instead of throwing.
  assert(isValidationError(await tools.roll_dice.execute({}, execution("call-3"))));
  // The model calls tools.execute naming the bundled tool: it runs here with the outer call's
  // requestContext, audited once.
  assert.deepEqual(
    await tools.tools_execute.execute(
      { name: "roll_dice", input: { sides: 4 } },
      execution("call-4"),
    ),
    { roll: 4 },
  );
  assert.deepEqual(users, ["ada", "ada"]);
  assert.deepEqual(
    ctx.reports.map((r) => [r.toolCallId, r.name, r.status, r.summary, r.display]),
    [
      ["call-1", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-1", "roll_dice", "completed", "Rolled a die", "summary"],
      ["call-2", "fail", "running", "Failed on purpose", undefined],
      ["call-2", "fail", "failed", "Failed on purpose", undefined],
      ["call-3", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-3", "roll_dice", "failed", "Rolled a die", "summary"],
      ["call-4", "roll_dice", "running", "Rolled a die", "summary"],
      ["call-4", "roll_dice", "completed", "Rolled a die", "summary"],
    ],
  );
  assert.deepEqual(ctx.reports[1].output, { roll: 6 });
  assert.match(ctx.reports[5].error, /validation failed/i);
  await assert.rejects(withTildeTools(ctx, { sendMessage: fail }), /conflicts with another tool/);
});

void test("withTildeTools with no tools unpublishes the last bundled tool", async () => {
  const ctx = context();
  const runs = [];
  const oldTool = createTool({
    id: "old_tool",
    description: "Removed later.",
    inputSchema: { type: "object" },
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
  const tools = convertToMastraTools({
    [STAGES]: catalogTool(calls, STAGES),
    [REGION]: catalogTool(calls, REGION),
  });
  const [stages, region] = Object.keys(tools);
  // The same alias in every adapter and in the Python SDK.
  assert.equal(stages, "customer_relationship_hub_list_open_opportunities_for_o_d9926af9");
  assert.equal(region.length, 64);
  assert.notEqual(stages, region);
  await tools[region].execute({}, { toolCallId: "call-1", messages: [] });
  assert.deepEqual(calls, [REGION]);
});
