import { BinaryPermission, TargetSelection } from "../dist/gen/tilde/types/v1/agent_pb.js";
import { startOidc, loginManagement } from "../../../../../scripts/test-oidc.mjs";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { once } from "node:events";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import {
  createManagementClient,
  createAgentServer,
  RuntimeChatService,
  AgentHostService,
} from "../dist/index.js";
import { createClient as rpcClient, Code } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-node";

assert(
  process.env.TEST_DATABASE_URL,
  "Run through scripts/with-postgres.sh; TEST_DATABASE_URL is required",
);
const oidc = await startOidc();
const binary = resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde");
const signingKey = randomBytes(32).toString("hex");
const seed = randomBytes(32).toString("base64");
let firstChunk, releaseChunk, releaseBroken;
const brokenGate = new Promise((r) => {
  releaseBroken = r;
});
const firstChunkSeen = new Promise((r) => {
  firstChunk = r;
});
const chunkGate = new Promise((r) => {
  releaseChunk = r;
});
const sessions = new Set();
const errors = [];
const contexts = [];
let goalId,
  taskId,
  firstRunId,
  leaveOnce = true,
  releaseUnconsumed;
const unconsumedGate = new Promise((r) => {
  releaseUnconsumed = r;
});
let healthReady = true;
let healthThrows = false;
const server = createAgentServer({
  signingKey,
  healthz() {
    if (healthThrows) throw new Error("private dependency failure");
    return healthReady;
  },
  async run(ctx) {
    contexts.push(ctx);
    assert.equal(ctx.tools.sendMessage.providerId, "native");
    assert.match(ctx.tools.sendMessage.description, /plain-text/);
    assert.equal(ctx.tools.sendMessage.chunkSchema.properties.textDelta.type, "string");
    assert.equal(ctx.tools["chat.send_message"], undefined);
    try {
      await ctx.reason("private reasoning, not a visible message");
      if (ctx.objective === "unconsumed") {
        if (leaveOnce) {
          leaveOnce = false;
          await unconsumedGate;
          return;
        }
        while (!ctx.signal.aborted) {
          const inputs = ctx.takeInputs();
          if (inputs.length) {
            assert.equal(inputs[0].text, "carry this forward");
            await ctx.sendNativeMessage("Carried into the next invocation");
            await ctx.setRunStatus("completed");
            return;
          }
          await delay(20, undefined, { signal: ctx.signal });
        }
      }
      if (ctx.objective === "cancel me") {
        await delay(10000, undefined, { signal: ctx.signal });
        return;
      }
      if (ctx.objective === "quiet") {
        await assert.rejects(
          ctx.tools.sendMessage.execute(
            { html: "not native input" },
            { toolCallId: "invalid-native-format" },
          ),
          (e) => e.code === Code.InvalidArgument,
        );
        await ctx.goals.create({ objective: "Keep working later" });
        ctx.stop();
      }
      if (ctx.objective === "other agent") {
        await assert.rejects(
          ctx.goals.update({ id: goalId, status: "completed" }),
          (e) => e.code === Code.FailedPrecondition,
        );
        await assert.rejects(
          ctx.tasks.update({ id: taskId, status: "completed" }),
          (e) => e.code === Code.FailedPrecondition,
        );
        assert.equal((await ctx.goals.list()).length, 0);
        await ctx.setRunStatus("completed");
        return;
      }
      if (ctx.objective === "stream") {
        firstRunId = ctx.runId;
        const goal = await ctx.goals.create({ objective: "Reply explicitly" });
        goalId = goal.id;
        const task = await ctx.tasks.create({ title: "Send reply", goalId });
        taskId = task.id;
        const dependent = await ctx.tasks.create({
          title: "Finish",
          goalId,
          dependencyIds: [taskId],
        });
        await assert.rejects(
          ctx.tasks.update({ id: dependent.id, status: "working" }),
          (e) => e.code === Code.FailedPrecondition,
        );
        await ctx.tasks.update({ id: taskId, status: "working" });
        const message = await ctx.sendNativeMessage(
          (async function* () {
            yield "Hello ";
            firstChunk();
            await chunkGate;
            yield "world";
          })(),
        );
        assert.equal(message.text, "Hello world");
        assert.equal(message.status, "complete");
        await ctx.tasks.update({ id: taskId, status: "completed" });
        await ctx.tasks.update({ id: dependent.id, status: "completed" });
        await assert.rejects(
          ctx.tasks.update({ id: taskId, status: "working" }),
          (e) => e.code === Code.FailedPrecondition,
        );
        await ctx.goals.update({ id: goalId, status: "completed" });
        await ctx.setRunStatus("completed");
        ctx.stop();
      }
      if (ctx.objective === "broken stream") {
        await assert.rejects(
          ctx.sendNativeMessage(
            (async function* () {
              yield "unfinished";
              await brokenGate;
              throw new Error("producer failed");
            })(),
          ),
        );
        return;
      }
      if (ctx.objective === "steer me") {
        while (!ctx.signal.aborted) {
          const inputs = ctx.takeInputs();
          if (inputs.length) {
            assert.equal(inputs.length, 1);
            await ctx.sendNativeMessage(inputs[0].text);
            await ctx.setRunStatus("completed");
            return;
          }
          await delay(20, undefined, { signal: ctx.signal });
        }
      }
    } catch (error) {
      if (error.name !== "StopLoop" && !ctx.signal.aborted) errors.push(error);
      throw error;
    }
  },
});
server.on("session", (session) => {
  sessions.add(session);
  session.on("close", () => sessions.delete(session));
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
const endpoint = `http://127.0.0.1:${server.address().port}`;
let logs = "";
let child;
async function start() {
  const env = {
    ...process.env,
    ...oidc.env,
    ENGINE_EVENT_INGRESS_LISTEN: "127.0.0.1:0",
    ENGINE_AGENT_RUNTIME_LISTEN: "127.0.0.1:0",
    DATABASE_URL: process.env.TEST_DATABASE_URL,
    ENGINE_ENCRYPTION_BACKEND: "seed",
    ENGINE_ENCRYPTION_KEY: seed,
    ENGINE_KMS_KEY_ID: "",
    RUST_LOG: "tilde=info",
    API_PORT: "18111",
    WEB_PORT: "18112",
  };
  delete env.ENGINE_MANAGEMENT_PUBLIC_URL;
  child = spawn(binary, ["--management-listen", "127.0.0.1:0"], {
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  return await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("Tilde did not start")), 15000);
    const data = (chunk) => {
      logs += chunk;
      const match = logs.match(/address=(127\.0\.0\.1:\d+)/);
      if (match) {
        clearTimeout(timer);
        resolve(`http://${match[1]}`);
      }
    };
    child.stdout.on("data", data);
    child.stderr.on("data", data);
    child.on("error", reject);
    child.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`Tilde exited ${code}: ${logs}`));
    });
  });
}
async function eventually(fn, timeout = 15000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const value = await fn();
    if (value) return value;
    await delay(50);
  }
  throw new Error("Timed out waiting for condition");
}
let watchAbort;
try {
  const url = await start();
  const client = createManagementClient({ baseUrl: url, accessToken: await loginManagement(url) });
  const runtime = rpcClient(
    AgentHostService,
    createConnectTransport({ baseUrl: endpoint, httpVersion: "2" }),
  );
  assert((await runtime.healthz({})).ready);
  healthReady = false;
  assert.equal((await runtime.healthz({})).ready, false);
  healthThrows = true;
  assert.equal((await runtime.healthz({})).ready, false);
  healthThrows = false;
  healthReady = true;
  await assert.rejects(
    runtime.cancel({ invocationId: randomUUID() }),
    (e) => e.code === Code.Unauthenticated,
  );
  const unbound = rpcClient(
    RuntimeChatService,
    createConnectTransport({
      baseUrl: `http://${logs.match(/agent_runtime_address=(127\.0\.0\.1:\d+)/)[1]}`,
      httpVersion: "2",
    }),
  );
  await assert.rejects(unbound.listGoals({}), (e) => e.code === Code.Unauthenticated);
  const alice = (await client.chat.createUser({ name: "Alice" })).user;
  const bob = (await client.chat.createUser({ name: "Bob" })).user;
  const agent = (
    await client.agents.createAgent({
      capabilities: {
        toolsInvoke: { mode: TargetSelection.ALL },
        workRead: BinaryPermission.YES,
        workWrite: BinaryPermission.YES,
        runUpdate: BinaryPermission.YES,
      },
      name: "Coordinator",
      endpointUrl: endpoint,
      webhookSigningKey: signingKey,
    })
  ).agent;
  const second = (
    await client.agents.createAgent({
      capabilities: {
        toolsInvoke: { mode: TargetSelection.ALL },
        workRead: BinaryPermission.YES,
        workWrite: BinaryPermission.YES,
        runUpdate: BinaryPermission.YES,
      },
      name: "Specialist",
      endpointUrl: endpoint,
      webhookSigningKey: signingKey,
    })
  ).agent;
  const thread = (
    await client.chat.createThread({
      title: "Group",
      primaryAgentId: agent.id,
      participants: [
        { userId: alice.id },
        { userId: bob.id },
        { agentId: agent.id },
        { agentId: second.id },
      ],
    })
  ).thread;
  assert.equal(thread.participants.length, 4);
  const single = (
    await client.chat.createThread({
      title: "Single",
      primaryAgentId: agent.id,
      participants: [{ userId: alice.id }, { agentId: agent.id }],
    })
  ).thread;
  assert.equal(single.participants.length, 2);
  const multi = (
    await client.chat.createThread({
      title: "Multi-agent",
      primaryAgentId: agent.id,
      participants: [{ userId: alice.id }, { agentId: agent.id }, { agentId: second.id }],
    })
  ).thread;
  assert.equal(multi.participants.length, 3);
  const aliceParticipant = thread.participants.find((p) => p.userId === alice.id);
  await assert.rejects(
    client.chat.postMessage({
      id: randomUUID(),
      threadId: thread.id,
      participantId: thread.participants.find((p) => p.agentId === agent.id).id,
      text: "forged agent",
    }),
    (e) => e.code === Code.Unauthenticated,
  );
  watchAbort = new AbortController();
  const events = [];
  const watcher = (async () => {
    try {
      for await (const response of client.chat.watchThread(
        { threadId: thread.id },
        { signal: watchAbort.signal },
      )) {
        const event = response.activity;
        events.push(event);
        if (event.textDelta === "unfinished") releaseBroken();
      }
    } catch (e) {
      if (!watchAbort.signal.aborted) throw e;
    }
  })();
  await client.chat.postMessage({
    id: randomUUID(),
    threadId: thread.id,
    participantId: aliceParticipant.id,
    text: "stream",
  });
  await Promise.race([
    firstChunkSeen,
    delay(15000).then(() => {
      throw new Error(`Agent stream did not start: ${logs}`);
    }),
  ]);
  const partial = await eventually(async () => {
    const list = (await client.chat.listMessages({ threadId: thread.id })).messages;
    return list.find((m) => m.status === "streaming" && m.text === "Hello ");
  });
  assert(partial);
  assert(!partial.text.includes("world"));
  // Other agents in this thread and this agent in another thread must not queue behind this stream.
  const parallel = (
    await client.chat.startRun({
      threadId: thread.id,
      agentId: second.id,
      objective: "other agent",
      idempotencyKey: randomUUID(),
    })
  ).run;
  await eventually(
    async () => (await client.chat.getRun({ id: parallel.id })).run.invocationStatus === "stopped",
  );
  const otherThread = (
    await client.chat.startRun({
      threadId: single.id,
      agentId: agent.id,
      objective: "quiet",
      idempotencyKey: randomUUID(),
    })
  ).run;
  await eventually(
    async () =>
      (await client.chat.getRun({ id: otherThread.id })).run.invocationStatus === "stopped",
  );
  await assert.rejects(
    client.chat.startRun({
      threadId: thread.id,
      agentId: agent.id,
      objective: "overlap",
      idempotencyKey: randomUUID(),
    }),
    (e) => e.code === Code.FailedPrecondition,
  );

  await eventually(() =>
    events.find((e) => e.kind === "message.delta" && e.textDelta === "Hello "),
  );
  releaseChunk();
  await eventually(async () => {
    const r = (await client.chat.getRun({ id: firstRunId })).run;
    return r.invocationStatus === "stopped" && r.status === "completed";
  });
  assert.equal(
    (await client.chat.listMessages({ threadId: thread.id })).messages.filter(
      (m) => m.status === "complete" && m.text === "Hello world",
    ).length,
    1,
  );
  assert(
    !(await client.chat.listMessages({ threadId: thread.id })).messages.some((m) =>
      m.text.includes("private reasoning"),
    ),
  );
  assert(events.some((e) => e.kind === "reasoning.delta"));
  assert(contexts[0].participants.some((p) => p.agentId === second.id));
  assert(contexts[0].tools.stop && contexts[0].tools["goals.create"]);
  await assert.rejects(contexts[0].goals.list());
  async function invoke(objective, agentId = agent.id) {
    return (
      await client.chat.startRun({
        threadId: thread.id,
        agentId,
        objective,
        idempotencyKey: randomUUID(),
      })
    ).run;
  }
  const other = await invoke("other agent", second.id);
  await eventually(
    async () => (await client.chat.getRun({ id: other.id })).run.invocationStatus === "stopped",
  );
  const quiet = await invoke("quiet");
  await eventually(
    async () => (await client.chat.getRun({ id: quiet.id })).run.invocationStatus === "stopped",
  );
  assert.equal((await client.chat.getRun({ id: quiet.id })).run.status, "waiting");
  const resumed = (await client.chat.resumeRun({ id: quiet.id })).run;
  assert.notEqual(resumed.invocationId, quiet.invocationId);
  await eventually(
    async () => (await client.chat.getRun({ id: quiet.id })).run.invocationStatus === "stopped",
  );
  const steer = await invoke("steer me");
  await eventually(() => contexts.find((c) => c.invocationId === steer.invocationId));
  const input = {
    invocationId: steer.invocationId,
    inputId: randomUUID(),
    text: "new direction",
  };
  await client.chat.steerInvocation(input);
  await client.chat.steerInvocation(input);
  await eventually(
    async () => (await client.chat.getRun({ id: steer.id })).run.invocationStatus === "stopped",
  );
  assert(
    (await client.chat.listMessages({ threadId: thread.id })).messages.some(
      (m) => m.text === "new direction",
    ),
  );
  const pending = await invoke("unconsumed");
  const pendingContext = await eventually(() =>
    contexts.find((c) => c.invocationId === pending.invocationId),
  );
  const carry = {
    invocationId: pending.invocationId,
    inputId: randomUUID(),
    text: "carry this forward",
  };
  await client.chat.steerInvocation(carry);
  await eventually(() => pendingContext.pendingInputIds().includes(carry.inputId));
  releaseUnconsumed();
  await eventually(async () => {
    const current = (await client.chat.getRun({ id: pending.id })).run;
    return (
      current.status === "completed" &&
      current.invocationStatus === "stopped" &&
      current.invocationId !== pending.invocationId
    );
  });
  const broken = await invoke("broken stream");
  await eventually(
    async () => (await client.chat.getRun({ id: broken.id })).run.invocationStatus === "stopped",
  );
  await eventually(async () =>
    (await client.chat.listMessages({ threadId: thread.id })).messages.some(
      (m) => m.status === "aborted" && m.text === "unfinished",
    ),
  );
  await eventually(() => events.some((e) => e.kind === "message.aborted"));
  const cancel = await invoke("cancel me");
  await eventually(() => contexts.find((c) => c.invocationId === cancel.invocationId));
  await client.chat.cancelInvocation({ invocationId: cancel.invocationId });
  assert.equal((await client.chat.getRun({ id: cancel.id })).run.invocationStatus, "canceled");
  await eventually(
    () => contexts.find((c) => c.invocationId === cancel.invocationId).signal.aborted,
  );
  const pauseRun = await invoke("cancel me");
  const pausedContext = await eventually(() =>
    contexts.find((c) => c.invocationId === pauseRun.invocationId),
  );
  const paused = await client.agents.pauseAgent({ id: agent.id });
  assert(paused.agent.paused && paused.stopAcknowledged);
  await eventually(() => pausedContext.signal.aborted);
  assert.equal((await client.chat.getRun({ id: pauseRun.id })).run.invocationStatus, "canceled");
  assert((await runtime.healthz({})).ready);
  await assert.rejects(invoke("cancel me"), (e) => e.code === Code.FailedPrecondition);
  assert.equal((await client.agents.resumeAgent({ id: agent.id })).agent.paused, false);
  const resumedRun = await invoke("cancel me");
  const resumedContext = await eventually(() =>
    contexts.find((c) => c.invocationId === resumedRun.invocationId),
  );
  assert(resumedContext.agentGeneration > pausedContext.agentGeneration);
  assert((await client.agents.pauseAgent({ id: agent.id })).stopAcknowledged);
  await eventually(() => resumedContext.signal.aborted);
  await client.agents.deleteAgent({ id: agent.id });
  await assert.rejects(client.agents.getAgent({ id: agent.id }), (e) => e.code === Code.NotFound);
  assert((await client.chat.listMessages({ threadId: thread.id })).messages.length > 0);
  watchAbort.abort();
  await watcher;
  assert.deepEqual(errors, []);
  assert(!logs.includes(signingKey));
  assert(!logs.includes(seed));
  console.log(
    "PASS: Rust/Node ConnectRPC, all thread topologies, streamed partial/final messages, goal/task isolation and dependencies, stop/resume, steering and cancellation.",
  );
} finally {
  await oidc.stop();
  releaseChunk?.();
  releaseUnconsumed?.();
  watchAbort?.abort();
  if (child && child.exitCode === null) {
    const exited = once(child, "exit");
    child.kill("SIGTERM");
    const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
    await exited;
    clearTimeout(timer);
  }
  for (const session of sessions) session.destroy();
  server.close();
}
