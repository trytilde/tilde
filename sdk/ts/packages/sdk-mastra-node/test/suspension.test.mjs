import assert from "node:assert/strict";
import { test } from "node:test";

import { MockLanguageModelV3 } from "ai/test";
import { Agent } from "@mastra/core/agent";
import { tildeMastra } from "../dist/index.js";

const usage = {
  inputTokens: { total: 1, noCache: 1, cacheRead: undefined, cacheWrite: undefined },
  outputTokens: { total: 1, text: 1, reasoning: undefined },
};
const deploy = {
  content: [{ type: "tool-call", toolCallId: "c1", toolName: "deploy", input: "{}" }],
  finishReason: { unified: "tool-calls", raw: undefined },
  usage,
  warnings: [],
};
const done = {
  content: [{ type: "text", text: "Deployed" }],
  finishReason: { unified: "stop", raw: undefined },
  usage,
  warnings: [],
};

/**
 * One invocation as the SDK core presents it to the adapter: `deploy` is a Tilde tool that answers
 * pending, and `suspend` saves the state and ends the invocation as the core does.
 */
function invocation(resumed) {
  const controller = new AbortController();
  const ctx = {
    signal: controller.signal,
    takeInputs: () => [],
    setBundledTools: async () => {},
    skills: { summary: async () => "", tools: () => ({}) },
    channel: {
      current: {
        deploy: {
          description: "Deploy",
          inputSchema: { type: "object", properties: {} },
          execute: async (_input, { toolCallId }) => {
            ctx.pending.push(toolCallId);
            return { status: "pending" };
          },
        },
      },
    },
    agentTools: {},
    pending: [],
    takeResumed() {
      const taken = resumed;
      resumed = undefined;
      return taken;
    },
    enableSuspension(snapshot) {
      ctx.snapshot = snapshot;
    },
    get shouldSuspend() {
      return ctx.pending.length > 0;
    },
    async suspend(state) {
      ctx.saved = state;
      controller.abort();
      throw new Error("suspended");
    },
  };
  return ctx;
}

const agent = (model) =>
  new Agent({ id: "shipper", name: "Shipper", instructions: "Ship it.", model });
const generate = async (model, prompt, ctx) =>
  agent(model).generate(prompt, { ...(await tildeMastra(ctx)), maxSteps: 5 });

void test("a pending tool call suspends the loop and the next invocation continues with its outcome", async () => {
  // The first invocation: the model calls deploy, which answers pending; before the next model
  // call the adapter saves the conversation and the invocation ends.
  const first = invocation();
  const before = new MockLanguageModelV3({ doGenerate: async () => deploy });
  // Mastra ends the loop as on a tripwire.
  assert.ok((await generate(before, "Deploy it", first)).tripwire);
  assert.equal(before.doGenerateCalls.length, 1, "no model call after the pending answer");
  assert.deepEqual(first.pending, ["c1"]);
  const saved = first.saved;
  // Mastra keeps a tool's result in the assistant message that called it.
  assert.deepEqual(
    saved.map((m) => m.role),
    ["user", "assistant"],
  );
  const call = saved[1].content.parts.find((p) => p.type === "tool-invocation").toolInvocation;
  assert.deepEqual([call.toolCallId, call.result], ["c1", { status: "pending" }]);
  assert.deepEqual(JSON.parse(JSON.stringify(saved)), saved, "the state is plain JSON");

  // The run's next invocation: its own messages give way to the saved conversation, with the
  // call's outcome in place of the pending answer.
  const second = invocation({
    state: saved,
    outcomes: new Map([["c1", { output: { version: 7 } }]]),
  });
  const after = new MockLanguageModelV3({ doGenerate: async () => done });
  const result = await generate(after, "(woken)", second);
  assert.equal(result.text, "Deployed");
  const prompt = after.doGenerateCalls[0].prompt;
  assert.deepEqual(
    prompt.map((m) => m.role),
    ["system", "user", "assistant", "tool"],
  );
  assert.equal(prompt[1].content[0].text, "Deploy it");
  assert.deepEqual(prompt[3].content[0].output, { type: "json", value: { version: 7 } });
  assert.equal(second.saved, undefined, "nothing pending, nothing saved");
});

void test("Tilde's deadline suspends at the next step boundary and a failed call resumes as an error", async () => {
  const first = invocation();
  const quick = {
    content: [{ type: "tool-call", toolCallId: "q1", toolName: "deploy", input: "{}" }],
    finishReason: { unified: "tool-calls", raw: undefined },
    usage,
    warnings: [],
  };
  // The deadline arrives while the model works; the step's tool answers pending too.
  const model = new MockLanguageModelV3({
    doGenerate: async () => {
      first.pending.push("deadline");
      return quick;
    },
  });
  await generate(model, "Deploy", first);
  const second = invocation({
    state: first.saved,
    outcomes: new Map([["q1", { error: "Tool call aborted" }]]),
  });
  const after = new MockLanguageModelV3({ doGenerate: async () => done });
  await generate(after, "(woken)", second);
  assert.deepEqual(after.doGenerateCalls[0].prompt.at(-1).content[0].output, {
    type: "json",
    value: { error: "Tool call aborted" },
  });
});
