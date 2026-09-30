import { connectAgentFixture } from "./connected-agent-fixture.mjs";
// Every callback is fresh; interruption stops SDK work and blocks token renewal.
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
  createRuntimeClient,
  BinaryPermission,
  TargetSelection,
} from "../dist/index.js";
import { AgentConcurrencyPolicy } from "../dist/management.js";
assert(process.env.TEST_DATABASE_URL, "Use scripts/with-postgres.sh");
const scratch = await mkdtemp(join(tmpdir(), "tilde-concurrency-"));
const contexts = [];
const errors = [];
const releases = new Map();
const agentOptions = {
  async run(ctx) {
    contexts.push(ctx);
    try {
      assert.deepEqual(ctx.takeInputs(), [], "Queued events stay at the runtime");
      if (ctx.objective === "first")
        await new Promise((resolve) => {
          releases.set(ctx.agentId, resolve);
          ctx.signal.addEventListener("abort", resolve, { once: true });
        });
      if (ctx.signal.aborted) return;
      const history = await ctx.message.history();
      if (ctx.objective === "second")
        assert(
          !history.items.some((item) => item.type === "message" && item.message.text === "third"),
          "A queued invocation must not read future queued messages",
        );
      await ctx.channel.current.sendMessage({ text: `reply: ${ctx.objective}` });
      await ctx.setRunStatus("completed");
    } catch (error) {
      if (!ctx.signal.aborted) errors.push(error);
      throw error;
    }
  },
};
const env = {
  ...process.env,
  DATABASE_URL: process.env.TEST_DATABASE_URL,
  LOGS_QUEUE_DIR: scratch,
  ENGINE_ENCRYPTION_BACKEND: "seed",
  ENGINE_ENCRYPTION_KEY: randomBytes(32).toString("base64"),
  ENGINE_KMS_KEY_ID: "",
  ENGINE_S3_BUCKET: "",
  ENGINE_SERVE: "all",
  ENGINE_WEB_ENABLED: "false",
  ENGINE_LISTEN: "127.0.0.1:0",
  RUST_LOG: "tilde=info",
};
for (const name of [
  "ENGINE_PUBLIC_URL",
  "ENGINE_RUNTIME_PUBLIC_URL",
  "ENGINE_INGRESS_PUBLIC_URL",
  "ENGINE_CONNECTION_SETUP_PUBLIC_URL",
  "ENGINE_CONNECTION_UI_DEV_URL",
])
  delete env[name];
const child = spawn(resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde"), [], {
  env,
  stdio: ["ignore", "pipe", "pipe"],
});
let logs = "";
child.stdout.on("data", (c) => (logs += c));
child.stderr.on("data", (c) => (logs += c));
async function eventually(fn) {
  for (let i = 0; i < 400; i++) {
    if (errors.length) throw errors[0];
    const result = await fn();
    if (result) return result;
    if (child.exitCode !== null) throw new Error(logs);
    await delay(25);
  }
  throw new Error("Concurrency test timed out");
}
const connectedFixtures = [];

try {
  const url = await eventually(() => {
    const match = logs.match(/ address=(127\.0\.0\.1:\d+)/);
    return match && `http://${match[1]}`;
  });
  const client = createManagementClient({ baseUrl: url });
  async function setup(policy) {
    const agent = (
      await client.agents.createAgent({
        name: "Policy fixture",
        concurrencyPolicy: policy,
        capabilities: {
          threadRead: BinaryPermission.YES,
          runUpdate: BinaryPermission.YES,
          toolsInvoke: { mode: TargetSelection.ALL },
        },
      })
    ).agent;
    connectedFixtures.push(
      await connectAgentFixture({ url, agentId: agent.id, options: agentOptions }),
    );
    assert.equal((await client.agents.getAgent({ id: agent.id })).agent.concurrencyPolicy, policy);
    const { token } = await client.deployments.issueIngressToken({ agentId: agent.id });
    const chat = createTildeChatClient({ baseUrl: url, agentId: agent.id, accessToken: token });
    const user = (await chat.createUser({ name: "Policy tester" })).user;
    const thread = (
      await chat.createThread({
        title: "Policy",
        primaryAgentId: agent.id,
        participants: [{ userId: user.id }, { agentId: agent.id }],
      })
    ).thread;
    const participantId = thread.participants.find((p) => p.userId === user.id).id;
    return {
      agent,
      thread,
      chat,
      post: (text) =>
        chat.postMessage({ id: randomUUID(), threadId: thread.id, participantId, text }),
    };
  }
  for (const policy of [
    AgentConcurrencyPolicy.QUEUE,
    AgentConcurrencyPolicy.QUEUE_AND_BATCH,
    AgentConcurrencyPolicy.INTERRUPT,
  ]) {
    const { agent, thread, post, chat } = await setup(policy);
    await post("first");
    const first = await eventually(() => contexts.find((ctx) => ctx.agentId === agent.id));
    await eventually(() => releases.has(agent.id));
    // Another agent/thread is independent of the blocked response.
    const other = await setup(AgentConcurrencyPolicy.QUEUE);
    await other.post("unrelated");
    await eventually(async () =>
      (await other.chat.listMessages({ threadId: other.thread.id })).messages.some(
        (m) => m.text === "reply: unrelated",
      ),
    );
    await post("second");
    if (policy === AgentConcurrencyPolicy.INTERRUPT) {
      await eventually(() => first.signal.aborted);
      const next = await eventually(() =>
        contexts.find((ctx) => ctx.agentId === agent.id && ctx.invocationId !== first.invocationId),
      );
      assert.equal(next.objective, "second");
      const revoked = createRuntimeClient({
        baseUrl: url,
        accessToken: first.traceAuthorization().slice(7),
      });
      await revoked.chat.listMessages({});
      await assert.rejects(
        revoked.chat.renewConnectToken({}),
        "Interrupted invocations cannot renew their still-valid token",
      );
      await eventually(async () =>
        (await chat.listMessages({ threadId: thread.id })).messages.some(
          (m) => m.text === "reply: second",
        ),
      );
      assert.equal(contexts.filter((ctx) => ctx.agentId === agent.id).length, 2);
    } else {
      await post("third");
      await delay(100);
      assert.equal(first.signal.aborted, false);
      assert.equal(contexts.filter((ctx) => ctx.agentId === agent.id).length, 1);
      releases.get(agent.id)();
      const expected =
        policy === AgentConcurrencyPolicy.QUEUE
          ? ["first", "second", "third"]
          : ["first", "second\nthird"];
      await eventually(async () =>
        (await chat.listMessages({ threadId: thread.id })).messages.some(
          (m) => m.text === `reply: ${expected.at(-1)}`,
        ),
      );
      const received = contexts.filter((ctx) => ctx.agentId === agent.id);
      assert.deepEqual(
        received.map((ctx) => ctx.objective),
        expected,
      );
      assert.equal(new Set(received.map((ctx) => ctx.invocationId)).size, expected.length);
    }
  }
  // Explicit steering also dispatches a fresh event and remains idempotent after interruption.
  const { agent, post, thread, chat } = await setup(AgentConcurrencyPolicy.INTERRUPT);
  await post("first");
  const original = await eventually(() => contexts.find((ctx) => ctx.agentId === agent.id));
  const event = { invocationId: original.invocationId, inputId: randomUUID(), text: "replacement" };
  await chat.steerInvocation(event);
  await chat.steerInvocation(event);
  await eventually(async () =>
    (await chat.listMessages({ threadId: thread.id })).messages.some(
      (m) => m.text === "reply: replacement",
    ),
  );
  assert(original.signal.aborted);
  assert.equal(
    contexts.filter((ctx) => ctx.agentId === agent.id && ctx.objective === "replacement").length,
    1,
  );
  assert.deepEqual(errors, []);
  console.log(
    "PASS: persisted queue/interrupt/batch policies, fresh invocations, scoped history, cancellation controls, renewal refusal, idempotent steering and independent threads.",
  );
} finally {
  await Promise.allSettled(connectedFixtures.map((connection) => connection.close()));
  for (const release of releases.values()) release();
  if (child.exitCode === null) {
    const done = once(child, "exit");
    child.kill("SIGTERM");
    const timeout = setTimeout(() => child.kill("SIGKILL"), 5000);
    await done;
    clearTimeout(timeout);
  }
  await rm(scratch, { recursive: true, force: true });
}
