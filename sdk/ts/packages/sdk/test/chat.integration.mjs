import { connectAgentFixture } from "./connected-agent-fixture.mjs";
import { BinaryPermission, TargetSelection } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { once } from "node:events";
import { resolve, join } from "node:path";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { setTimeout as delay } from "node:timers/promises";
import {
  createManagementClient,
  createTildeChatClient,
  RuntimeChatService,
} from "../dist/index.js";
import { createClient as rpcClient, Code } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-node";

assert(
  process.env.TEST_DATABASE_URL,
  "Run through scripts/with-postgres.sh; TEST_DATABASE_URL is required",
);
const scratch = await mkdtemp(join(tmpdir(), "tilde-sdk-chat-"));
const binary = resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde");
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
const errors = [];
const contexts = [];
const checkpoints = new Set();
let goalId, taskId, firstRunId;
let hostReady = true;
const agentOptions = {
  async checkpoint(ctx) {
    checkpoints.add(ctx.invocationId);
  },
  ready: () => hostReady,
  async run(ctx) {
    contexts.push(ctx);
    assert.equal(ctx.tools.sendMessage.providerId, "native");
    assert.match(ctx.tools.sendMessage.description, /plain-text/);
    assert.equal(ctx.tools.sendMessage.chunkSchema.properties.textDelta.type, "string");
    assert.equal(ctx.tools["chat.send_message"], undefined);
    try {
      await ctx.reason("private reasoning, not a visible message");
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
    } catch (error) {
      if (error.name !== "StopLoop" && !ctx.signal.aborted) errors.push(error);
      throw error;
    }
  },
};
let logs = "";
let child;
async function start() {
  const env = {
    ...process.env,
    DATABASE_URL: process.env.TEST_DATABASE_URL,
    LOGS_QUEUE_DIR: join(scratch, "logs"),
    ENGINE_ENCRYPTION_BACKEND: "seed",
    ENGINE_ENCRYPTION_KEY: seed,
    ENGINE_KMS_KEY_ID: "",
    RUST_LOG: "tilde=info",
    API_PORT: "18111",
    WEB_PORT: "18112",
  };
  delete env.ENGINE_PUBLIC_URL;
  child = spawn(binary, ["--listen", "127.0.0.1:0"], {
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  return await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("Tilde did not start")), 15000);
    const data = (chunk) => {
      logs += chunk;
      const match = logs.match(/ address=(127\.0\.0\.1:\d+)/);
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
const connectedFixtures = [];

try {
  const url = await start();
  const client = createManagementClient({ baseUrl: url });
  const unbound = rpcClient(
    RuntimeChatService,
    createConnectTransport({
      baseUrl: url,
      httpVersion: "2",
    }),
  );
  await assert.rejects(unbound.listGoals({}), (e) => e.code === Code.Unauthenticated);
  const agent = (
    await client.agents.createAgent({
      capabilities: {
        toolsInvoke: { mode: TargetSelection.ALL },
        workRead: BinaryPermission.YES,
        workWrite: BinaryPermission.YES,
        runUpdate: BinaryPermission.YES,
      },
      name: "Coordinator",
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
    })
  ).agent;
  for (const current of [agent, second])
    connectedFixtures.push(
      await connectAgentFixture({ url, agentId: current.id, options: agentOptions }),
    );
  // Readiness reaches the registry only through heartbeats from the connected instance.
  const instanceReady = async () =>
    (await client.deployments.getDeployment({ agentId: agent.id })).instances.find(
      (i) => i.instanceId === connectedFixtures[0].instanceId,
    )?.ready;
  await eventually(instanceReady);
  hostReady = false;
  await eventually(async () => (await instanceReady()) === false);
  hostReady = true;
  await eventually(instanceReady);
  const { token } = await client.deployments.issueIngressToken({ agentId: agent.id });
  const chat = createTildeChatClient({ baseUrl: url, agentId: agent.id, accessToken: token });
  const alice = (await chat.createUser({ name: "Alice" })).user;
  const bob = (await chat.createUser({ name: "Bob" })).user;
  const thread = (
    await chat.createThread({
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
    await chat.createThread({
      title: "Single",
      primaryAgentId: agent.id,
      participants: [{ userId: alice.id }, { agentId: agent.id }],
    })
  ).thread;
  assert.equal(single.participants.length, 2);
  const multi = (
    await chat.createThread({
      title: "Multi-agent",
      primaryAgentId: agent.id,
      participants: [{ userId: alice.id }, { agentId: agent.id }, { agentId: second.id }],
    })
  ).thread;
  assert.equal(multi.participants.length, 3);
  const aliceParticipant = thread.participants.find((p) => p.userId === alice.id);
  await assert.rejects(
    chat.postMessage({
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
      for await (const response of chat.watchThread(
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
  await chat.postMessage({
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
    const list = (await chat.listMessages({ threadId: thread.id })).messages;
    return list.find((m) => m.status === "streaming" && m.text === "Hello ");
  });
  assert(partial);
  assert(!partial.text.includes("world"));
  // The UI bootstraps text and its replay cursor from one SQL snapshot.
  const bootstrap = await chat.getSession({ threadId: thread.id, includeHistory: true });
  const initial = bootstrap.messages.find((message) => message.id === partial.id);
  assert.equal(initial.text, "Hello ");
  const bootstrapReplay = (async () => {
    let text = initial.text;
    for await (const { activity } of chat.watchThread(
      { threadId: thread.id, afterCursor: bootstrap.cursor },
      { signal: AbortSignal.timeout(15000) },
    )) {
      if (
        activity?.detail.case === "messageChunk" &&
        activity.detail.value.messageId === partial.id
      )
        text += activity.detail.value.textDelta;
      if (
        activity?.detail.case === "message" &&
        activity.detail.value.id === partial.id &&
        activity.detail.value.status === "complete"
      ) {
        assert.equal(
          text,
          activity.detail.value.text,
          "snapshot plus replay must neither skip nor duplicate deltas",
        );
        return text;
      }
    }
    throw new Error("Bootstrap replay ended before the message completed");
  })();

  // Other agents in this thread and this agent in another thread must not queue behind this stream.
  const parallel = (
    await chat.startRun({
      threadId: thread.id,
      agentId: second.id,
      objective: "other agent",
      idempotencyKey: randomUUID(),
    })
  ).run;
  await eventually(
    async () => (await chat.getRun({ id: parallel.id })).run.invocationStatus === "stopped",
  );
  const otherThread = (
    await chat.startRun({
      threadId: single.id,
      agentId: agent.id,
      objective: "quiet",
      idempotencyKey: randomUUID(),
    })
  ).run;
  await eventually(
    async () => (await chat.getRun({ id: otherThread.id })).run.invocationStatus === "stopped",
  );
  await assert.rejects(
    chat.startRun({
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
  assert.equal(await bootstrapReplay, "Hello world");
  await eventually(async () => {
    const r = (await chat.getRun({ id: firstRunId })).run;
    return r.invocationStatus === "stopped" && r.status === "completed";
  });
  assert.equal(
    (await chat.listMessages({ threadId: thread.id })).messages.filter(
      (m) => m.status === "complete" && m.text === "Hello world",
    ).length,
    1,
  );
  assert(
    !(await chat.listMessages({ threadId: thread.id })).messages.some((m) =>
      m.text.includes("private reasoning"),
    ),
  );
  assert(events.some((e) => e.kind === "reasoning.delta"));
  assert(contexts[0].participants.some((p) => p.agentId === second.id));
  assert(contexts[0].tools.stop && contexts[0].tools["goals.create"]);
  await assert.rejects(contexts[0].goals.list());
  async function invoke(objective, agentId = agent.id) {
    return (
      await chat.startRun({
        threadId: thread.id,
        agentId,
        objective,
        idempotencyKey: randomUUID(),
      })
    ).run;
  }
  const other = await invoke("other agent", second.id);
  await eventually(
    async () => (await chat.getRun({ id: other.id })).run.invocationStatus === "stopped",
  );
  const quiet = await invoke("quiet");
  await eventually(
    async () => (await chat.getRun({ id: quiet.id })).run.invocationStatus === "stopped",
  );
  assert.equal((await chat.getRun({ id: quiet.id })).run.status, "waiting");
  const resumed = (await chat.resumeRun({ id: quiet.id })).run;
  assert.notEqual(resumed.invocationId, quiet.invocationId);
  await eventually(
    async () => (await chat.getRun({ id: quiet.id })).run.invocationStatus === "stopped",
  );
  const broken = await invoke("broken stream");
  await eventually(
    async () => (await chat.getRun({ id: broken.id })).run.invocationStatus === "stopped",
  );
  await eventually(async () =>
    (await chat.listMessages({ threadId: thread.id })).messages.some(
      (m) => m.status === "aborted" && m.text === "unfinished",
    ),
  );
  await eventually(() => events.some((e) => e.kind === "message.aborted"));
  const cancel = await invoke("cancel me");
  await eventually(() => contexts.find((c) => c.invocationId === cancel.invocationId));
  await chat.cancelInvocation({ invocationId: cancel.invocationId });
  assert.equal((await chat.getRun({ id: cancel.id })).run.invocationStatus, "canceled");
  await eventually(
    () => contexts.find((c) => c.invocationId === cancel.invocationId).signal.aborted,
  );
  const suspension = await invoke("cancel me");
  await eventually(() => contexts.find((c) => c.invocationId === suspension.invocationId));
  await chat.suspendInvocation({ invocationId: suspension.invocationId });
  await eventually(() => checkpoints.has(suspension.invocationId));
  await eventually(
    async () => (await chat.getRun({ id: suspension.id })).run.invocationStatus === "stopped",
  );
  assert.equal((await chat.getRun({ id: suspension.id })).run.status, "waiting");
  const continuation = (await chat.resumeRun({ id: suspension.id })).run;
  assert.notEqual(continuation.invocationId, suspension.invocationId);
  await eventually(() => contexts.find((c) => c.invocationId === continuation.invocationId));
  await chat.cancelInvocation({ invocationId: continuation.invocationId });
  const pauseRun = await invoke("cancel me");
  const pausedContext = await eventually(() =>
    contexts.find((c) => c.invocationId === pauseRun.invocationId),
  );
  const paused = await client.agents.pauseAgent({ id: agent.id });
  assert(paused.agent.paused);
  await eventually(() => pausedContext.signal.aborted);
  assert.equal((await chat.getRun({ id: pauseRun.id })).run.invocationStatus, "canceled");
  await assert.rejects(invoke("cancel me"), (e) => e.code === Code.FailedPrecondition);
  assert.equal((await client.agents.resumeAgent({ id: agent.id })).agent.paused, false);
  const resumedRun = await invoke("cancel me");
  const resumedContext = await eventually(() =>
    contexts.find((c) => c.invocationId === resumedRun.invocationId),
  );
  assert(resumedContext.agentGeneration > pausedContext.agentGeneration);
  assert((await client.agents.pauseAgent({ id: agent.id })).agent.paused);
  await eventually(() => resumedContext.signal.aborted);
  await client.agents.deleteAgent({ id: agent.id });
  await assert.rejects(client.agents.getAgent({ id: agent.id }), (e) => e.code === Code.NotFound);
  await assert.rejects(chat.listMessages({ threadId: thread.id }), (e) => e.code === Code.NotFound);
  // Deleting an agent revokes its provider ingress, but shared history survives
  // and remains available through another participating agent's provider.
  const { token: secondToken } = await client.deployments.issueIngressToken({ agentId: second.id });
  const secondChat = createTildeChatClient({
    baseUrl: url,
    agentId: second.id,
    accessToken: secondToken,
  });
  assert((await secondChat.listMessages({ threadId: thread.id })).messages.length > 0);
  await assert.rejects(
    secondChat.listMessages({ threadId: single.id }),
    (e) => e.code === Code.PermissionDenied,
  );
  watchAbort.abort();
  await watcher;
  assert.deepEqual(errors, []);
  assert(!logs.includes(seed));
  console.log(
    "PASS: Rust/Node ConnectRPC, all thread topologies, streamed partial/final messages, goal/task isolation and dependencies, stop/resume and cancellation.",
  );
} finally {
  await Promise.allSettled(connectedFixtures.map((connection) => connection.close()));
  releaseChunk?.();
  watchAbort?.abort();
  if (child && child.exitCode === null) {
    const exited = once(child, "exit");
    child.kill("SIGTERM");
    const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
    await exited;
    clearTimeout(timer);
  }
  await rm(scratch, { recursive: true, force: true });
}
