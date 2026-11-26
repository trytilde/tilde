import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http2";
import { once } from "node:events";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import { createLambdaHandler, RuntimeChatService, RunService } from "../dist/index.js";
import {
  InvocationControlService,
  InvocationCommandKind as Kind,
} from "@trytilde/contracts/tilde/runtime/v1/controls_pb.js";

/**
 * Tilde as an invocation sees it: `ops.deploy` answers pending to a caller that can suspend,
 * SuspendRun keeps the run's checkpoint and GetCheckpoint returns it with the call's outcome;
 * `deadline` makes the control stream deliver a suspend.
 */
async function tilde(t) {
  const state = { calls: [], saved: undefined, reports: [], deadline: false, acknowledged: [] };
  const server = createServer(
    connectNodeAdapter({
      routes(router) {
        router.service(RuntimeChatService, {
          async listTools() {
            return {
              tools: [
                {
                  name: "ops.deploy",
                  providerId: "ops",
                  description: "Deploy",
                  inputSchemaJson: '{"type":"object"}',
                },
              ],
            };
          },
          async registerBundledTools() {
            return {};
          },
          async *invokeToolStream(requests) {
            for await (const frame of requests) {
              state.calls.push(frame);
              const pending = frame.suspendable;
              yield {
                frame: {
                  case: "result",
                  value: {
                    outputJson: JSON.stringify(pending ? { status: "pending" } : { version: 6 }),
                    pending,
                  },
                },
              };
              return;
            }
          },
          async suspendRun(request) {
            state.saved = request;
            return {};
          },
          async getCheckpoint() {
            return {
              stateJson: state.saved.stateJson,
              results: state.saved.pendingCallIds.map((id) => ({
                id,
                status: "completed",
                outputJson: '{"version":7}',
              })),
            };
          },
        });
        router.service(RunService, {
          async report(request) {
            state.reports.push(request.event.case);
            return {};
          },
        });
        router.service(InvocationControlService, {
          async *watchCommands(_, ctx) {
            yield { kind: Kind.READY };
            // The deadline comes while the loop runs.
            if (state.deadline) {
              await delay(100);
              yield { id: randomUUID(), kind: Kind.SUSPEND };
            }
            await new Promise((resolve) =>
              ctx.signal.addEventListener("abort", resolve, { once: true }),
            );
          },
          async acknowledgeCommand(request) {
            state.acknowledged.push(request.id);
            return {};
          },
        });
      },
    }),
  );
  const sessions = new Set();
  server.on("session", (session) => {
    sessions.add(session);
    session.on("close", () => sessions.delete(session));
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => {
    for (const session of sessions) session.destroy();
    server.close();
  });
  const wake = (extra = {}) => ({
    agentId: randomUUID(),
    threadId: randomUUID(),
    invocationId: randomUUID(),
    runId: randomUUID(),
    capability: "capability",
    callbackUrl: `http://127.0.0.1:${server.address().port}`,
    ...extra,
  });
  return { state, wake };
}

void test("a pending call suspends the run with its state and the next invocation reads the outcome", async (t) => {
  const { state, wake } = await tilde(t);
  let latest = [];
  // An adapter's loop: one tool call per step, checking for suspension at each step boundary.
  const handler = createLambdaHandler({
    tracing: "existing",
    async run(ctx) {
      const resumed = ctx.takeResumed();
      if (resumed) {
        state.resumed = resumed;
        assert.equal(ctx.takeResumed(), undefined, "handed out once");
        return;
      }
      ctx.enableSuspension(() => latest);
      latest = [{ role: "user", content: "Deploy" }];
      const output = await ctx.tools["ops.deploy"].execute(
        {},
        { toolCallId: "c1", suspendable: true },
      );
      latest = [...latest, { role: "tool", toolCallId: "c1", output }];
      assert(ctx.shouldSuspend);
      await ctx.suspend(latest);
      throw new Error("suspend does not return");
    },
  });
  await handler(wake());
  assert.equal(state.calls[0].suspendable, true);
  const saved = JSON.parse(state.saved.stateJson);
  assert.equal(state.saved.pendingCallIds.length, 1);
  assert.deepEqual(saved.state, latest);
  assert.deepEqual(saved.pending, [
    { toolCallId: "c1", callId: state.saved.pendingCallIds[0], name: "ops.deploy" },
  ]);
  // Suspension is the run's end for this invocation, not a failure.
  assert.deepEqual(state.reports, ["stopped"]);

  await handler(wake({ resumes: true }));
  assert.deepEqual(state.resumed.state, latest);
  assert.deepEqual([...state.resumed.outcomes], [["c1", { output: { version: 7 } }]]);
});

void test("Tilde's deadline asks the loop to suspend, and a loop that ends with a call pending is suspended", async (t) => {
  const { state, wake } = await tilde(t);
  state.deadline = true;
  const handler = createLambdaHandler({
    tracing: "existing",
    async run(ctx) {
      ctx.enableSuspension(() => ["boundary"]);
      for (let i = 0; i < 200 && !ctx.shouldSuspend; i++) await delay(10);
      assert(ctx.shouldSuspend, "the suspend command reached the loop");
      await ctx.suspend(["deadline"]);
    },
  });
  await handler(wake());
  assert.deepEqual(JSON.parse(state.saved.stateJson), { state: ["deadline"], pending: [] });
  assert.equal(state.acknowledged.length, 1);

  // Without a step boundary after its pending call the loop just returns: its last state is saved.
  state.deadline = false;
  state.saved = undefined;
  await createLambdaHandler({
    tracing: "existing",
    async run(ctx) {
      ctx.enableSuspension(() => ["last boundary"]);
      await ctx.tools["ops.deploy"].execute({}, { toolCallId: "c2", suspendable: true });
    },
  })(wake());
  assert.deepEqual(JSON.parse(state.saved.stateJson).state, ["last boundary"]);

  // A loop that cannot suspend never sees pending: its calls block as before.
  state.calls = [];
  let output;
  await createLambdaHandler({
    tracing: "existing",
    async run(ctx) {
      output = await ctx.tools["ops.deploy"].execute({}, { toolCallId: "c3" });
    },
  })(wake());
  assert.equal(state.calls[0].suspendable, false);
  assert.deepEqual(output, { version: 6 });
});
