import assert from "node:assert/strict";
import { test } from "node:test";
import { RunContext } from "@openai/agents";
import { convertToOpenAIAgentsTools } from "../dist/index.js";

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
  assert.deepEqual(calls, [{ input: { value: 3 }, execution: { toolCallId: "call-123" } }]);
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
