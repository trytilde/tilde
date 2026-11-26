import assert from "node:assert/strict";
import { test } from "node:test";
import { ToolLoopAgent, generateText, stepCountIs } from "ai";
import { MockLanguageModelV3 } from "ai/test";
import {
  prepareTildeCall,
  tildeAiSdk,
  tildeCallOptions,
  tildeConversation,
} from "../dist/index.js";

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
  new ToolLoopAgent({
    model,
    instructions: "Ship it.",
    stopWhen: stepCountIs(5),
    ...tildeCallOptions,
  });

void test("a pending tool call suspends the loop and the next invocation continues with its outcome", async () => {
  // The first invocation: the model calls deploy, which answers pending; before the next model
  // call the adapter saves the conversation and the invocation ends.
  const first = invocation();
  const before = new MockLanguageModelV3({ doGenerate: async () => deploy });
  await assert.rejects(
    agent(before).generate({ prompt: "Deploy it", ...tildeAiSdk(first) }),
    /suspended/,
  );
  assert.equal(before.doGenerateCalls.length, 1, "no model call after the pending answer");
  assert.deepEqual(first.pending, ["c1"]);
  const saved = first.saved;
  assert.deepEqual(
    saved.map((m) => m.role),
    ["user", "assistant", "tool"],
  );
  assert.deepEqual(JSON.parse(JSON.stringify(saved)), saved, "the state is plain JSON");

  // The run's next invocation: its own messages give way to the saved conversation, with the
  // call's outcome in place of the pending answer.
  const second = invocation({
    state: saved,
    outcomes: new Map([["c1", { output: { version: 7 } }]]),
  });
  const after = new MockLanguageModelV3({ doGenerate: async () => done });
  const result = await agent(after).generate({ prompt: "(woken)", ...tildeAiSdk(second) });
  assert.equal(result.text, "Deployed");
  const prompt = after.doGenerateCalls[0].prompt;
  assert.deepEqual(
    prompt.map((m) => m.role),
    ["system", "user", "assistant", "tool"],
  );
  assert.equal(prompt[1].content[0].text, "Deploy it");
  assert.deepEqual(prompt[3].content[0].output, { type: "json", value: { version: 7 } });
  assert.equal(second.saved, undefined, "nothing pending, nothing saved");
  // The loop is over: a forced suspension now would resume a finished conversation.
  assert.equal(second.snapshot(), undefined);
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
  await assert.rejects(agent(model).generate({ prompt: "Deploy", ...tildeAiSdk(first) }));
  const second = invocation({
    state: first.saved,
    outcomes: new Map([["q1", { error: "Tool call aborted" }]]),
  });
  const after = new MockLanguageModelV3({ doGenerate: async () => done });
  await agent(after).generate({ prompt: "(woken)", ...tildeAiSdk(second) });
  assert.deepEqual(after.doGenerateCalls[0].prompt.at(-1).content[0].output, {
    type: "error-text",
    value: "Tool call aborted",
  });
});

void test("a loop that stops on a pending step saves that step, with the default stop too", async () => {
  // A quick call, then a pending one on the last allowed step.
  const quick = {
    ...deploy,
    content: [{ type: "tool-call", toolCallId: "q0", toolName: "noop", input: "{}" }],
  };
  const first = invocation();
  first.channel.current.noop = {
    description: "Noop",
    inputSchema: { type: "object", properties: {} },
    execute: async () => ({ ok: true }),
  };
  const replies = [quick, deploy];
  const model = new MockLanguageModelV3({ doGenerate: async () => replies.shift() });
  const twice = new ToolLoopAgent({ model, stopWhen: stepCountIs(2), ...tildeCallOptions });
  await twice.generate({ prompt: "Deploy it", ...tildeAiSdk(first) });
  assert.deepEqual(
    first.snapshot().map((m) => m.role),
    ["user", "assistant", "tool", "assistant", "tool"],
  );
  assert.deepEqual(JSON.parse(JSON.stringify(tildeConversation(first))), first.snapshot());
  // generateText's own default, one step: the pending step is still the snapshot.
  const second = invocation();
  const settings = await prepareTildeCall(second, {
    model: new MockLanguageModelV3({ doGenerate: async () => deploy }),
    prompt: "Deploy it",
  });
  await generateText(settings);
  assert.deepEqual(
    second.snapshot().map((m) => m.role),
    ["user", "assistant", "tool"],
  );
});

void test("the saved conversation is handed out once: a second loop starts from its own messages", async () => {
  const saved = [{ role: "user", content: [{ type: "text", text: "Saved" }] }];
  const ctx = invocation({ state: saved, outcomes: new Map() });
  const seen = [];
  const model = new MockLanguageModelV3({
    doGenerate: async ({ prompt }) => {
      seen.push(prompt.filter((m) => m.role === "user").map((m) => m.content[0].text));
      return done;
    },
  });
  await agent(model).generate({ prompt: "first", ...tildeAiSdk(ctx) });
  await agent(model).generate({ prompt: "second", ...tildeAiSdk(ctx) });
  assert.deepEqual(seen, [["Saved"], ["second"]]);
});
