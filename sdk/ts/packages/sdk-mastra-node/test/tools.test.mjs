import assert from "node:assert/strict";
import { test } from "node:test";
import { isValidationError } from "@mastra/core/tools";
import { convertToMastraTools } from "../dist/index.js";

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
  assert.deepEqual(calls, [{ input: { value: 3 }, execution: { toolCallId: "call-123" } }]);
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
