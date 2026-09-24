import assert from "node:assert/strict";
import { test } from "node:test";
import { convertToAiSdkTools } from "../dist/index.js";

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
  assert.deepEqual(calls, [{ input: { value: 3 }, execution: { toolCallId: "call-123" } }]);
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
