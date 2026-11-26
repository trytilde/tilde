import assert from "node:assert/strict";
import { test } from "node:test";
import { AIMessage, createAgent, fakeModel } from "langchain";
import { tildeLangChain, tildeMiddleware } from "../dist/index.js";

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
  createAgent({ model, systemPrompt: "Ship it.", middleware: [tildeMiddleware()] });

void test("a pending tool call suspends the graph and the next invocation continues with its outcome", async () => {
  const before = fakeModel().respondWithTools([{ name: "deploy", args: {}, id: "c1" }]);
  const first = invocation();
  await assert.rejects(
    agent(before).invoke(
      { messages: [{ role: "user", content: "Deploy it" }] },
      tildeLangChain(first),
    ),
  );
  assert.equal(before.calls.length, 1, "no model call after the pending answer");
  assert.deepEqual(
    first.saved.map((m) => m.type),
    ["human", "ai", "tool"],
  );

  const after = fakeModel().respond(new AIMessage("Deployed"));
  const second = invocation({
    state: first.saved,
    outcomes: new Map([["c1", { output: { version: 7 } }]]),
  });
  const result = await agent(after).invoke(
    { messages: [{ role: "user", content: "(woken)" }] },
    tildeLangChain(second),
  );
  const seen = after.calls[0].messages;
  assert.deepEqual(
    seen.map((m) => m.type),
    ["system", "human", "ai", "tool"],
  );
  assert.equal(seen[1].content, "Deploy it", "the saved conversation, not the call's own");
  assert.equal(seen[3].content, JSON.stringify({ version: 7 }));
  assert.equal(result.messages.at(-1).content, "Deployed");
});

void test("a failed pending call resumes as an error tool message", async () => {
  const first = invocation();
  await assert.rejects(
    agent(fakeModel().respondWithTools([{ name: "deploy", args: {}, id: "c2" }])).invoke(
      { messages: [{ role: "user", content: "Deploy" }] },
      tildeLangChain(first),
    ),
  );
  const after = fakeModel().respond(new AIMessage("Sorry"));
  const second = invocation({
    state: first.saved,
    outcomes: new Map([["c2", { error: "Tool call aborted" }]]),
  });
  await agent(after).invoke({ messages: [] }, tildeLangChain(second));
  const tool = after.calls[0].messages.at(-1);
  assert.equal(tool.content, "Tool call aborted");
  assert.equal(tool.status, "error");
});
