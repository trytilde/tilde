import assert from "node:assert/strict";
import { test } from "node:test";
import { ToolMessage } from "@langchain/core/messages";
import { convertToLangChainTools } from "../dist/index.js";

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
  assert.deepEqual(calls, [{ input: { value: 3 }, execution: { toolCallId: "call-123" } }]);
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
