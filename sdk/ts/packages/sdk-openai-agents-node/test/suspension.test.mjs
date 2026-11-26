import assert from "node:assert/strict";
import { test } from "node:test";
import { Agent, Usage, run, setTracingDisabled } from "@openai/agents";
import { tildeOpenAIAgents } from "../dist/index.js";

setTracingDisabled(true);

/** A model answering with scripted output items, recording each request's input. */
function scripted(...turns) {
  const inputs = [];
  return {
    inputs,
    async getResponse(request) {
      inputs.push(structuredClone(request.input));
      return { usage: new Usage(), output: turns.shift(), responseId: `r${inputs.length}` };
    },
    getStreamedResponse() {
      throw new Error("not streamed");
    },
  };
}
const callDeploy = (callId) => [
  {
    type: "function_call",
    id: `fc_${callId}`,
    callId,
    name: "deploy",
    arguments: "{}",
    status: "completed",
  },
];
const say = (text) => [
  {
    type: "message",
    id: "m1",
    role: "assistant",
    status: "completed",
    content: [{ type: "output_text", text }],
  },
];

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
    activatePrompt: () => {},
    skills: { summary: async () => "", tools: () => ({}), list: async () => [] },
    channel: {
      current: {
        deploy: {
          description: "Deploy",
          inputSchema: { type: "object", properties: {}, additionalProperties: false },
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

async function runOnce(model, ctx, input) {
  const tilde = await tildeOpenAIAgents(
    ctx,
    new Agent({ name: "shipper", instructions: "Ship it.", model }),
  );
  return run(tilde.agent, input, { ...tilde.options, maxTurns: 5 });
}

void test("a pending tool call suspends the run and the next invocation continues with its outcome", async () => {
  const first = invocation();
  const before = scripted(callDeploy("c1"), say("never"));
  await assert.rejects(runOnce(before, first, [{ role: "user", content: "Deploy it" }]));
  assert.equal(before.inputs.length, 1, "no model call after the pending answer");
  assert.deepEqual(
    first.saved.map((item) => item.type ?? item.role),
    ["user", "function_call", "function_call_result"],
  );

  const second = invocation({
    state: first.saved,
    outcomes: new Map([["c1", { output: { version: 7 } }]]),
  });
  const after = scripted(say("Deployed"));
  const result = await runOnce(after, second, [{ role: "user", content: "(woken)" }]);
  assert.equal(result.finalOutput, "Deployed");
  const [input] = after.inputs;
  assert.equal(input[0].content, "Deploy it", "the saved items, not the run's own input");
  assert.deepEqual(input.at(-1).output, { type: "text", text: JSON.stringify({ version: 7 }) });
});

void test("a failed pending call resumes as its error", async () => {
  const first = invocation();
  await assert.rejects(
    runOnce(scripted(callDeploy("c2")), first, [{ role: "user", content: "Deploy" }]),
  );
  const second = invocation({
    state: first.saved,
    outcomes: new Map([["c2", { error: "Tool call aborted" }]]),
  });
  const after = scripted(say("Sorry"));
  await runOnce(after, second, "(woken)");
  assert.deepEqual(after.inputs[0].at(-1).output, {
    type: "text",
    text: "Error: Tool call aborted",
  });
});
