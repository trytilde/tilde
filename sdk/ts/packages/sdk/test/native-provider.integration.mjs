import { connectAgentFixture } from "./connected-agent-fixture.mjs";
// Provider key -> authenticated proxy identity -> membership, search, reads and queue dispatch.
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
  BinaryPermission,
  TargetSelection,
} from "../dist/index.js";
import { createChatProxy } from "../../chat-proxy/dist/index.js";
import { createChatRouteHandlers } from "../../chat-next/dist/index.js";
import { createClient, Code } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ChatService } from "@trytilde/contracts/tilde/provider/tilde/v1/chat_pb.js";
import { AgentConcurrencyPolicy, ChannelAccessMode } from "../dist/management.js";
import { startOidc, loginManagement } from "../../../../../scripts/test-oidc.mjs";
assert(process.env.TEST_DATABASE_URL, "Use scripts/with-postgres.sh");
const scratch = await mkdtemp(join(tmpdir(), "tilde-concurrency-"));
const oidc = await startOidc();
const contexts = [];
const errors = [];
const releases = new Map();
const agentOptions = {
  async checkpoint() {},
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
      if (ctx.objective === "fourth") {
        const history = await ctx.message.history();
        assert(
          !history.items.some(
            (item) => item.type === "message" && ["second", "third"].includes(item.message.text),
          ),
          "reordered dispatch excludes pending and removed inputs",
        );
      }
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
  ...oidc.env,
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
  const managementToken = await loginManagement(url);
  const management = createManagementClient({
    baseUrl: url,
    accessToken: managementToken,
  });
  const { agent } = await management.agents.createAgent({
    name: "Embedded",
    concurrencyPolicy: AgentConcurrencyPolicy.QUEUE,
    capabilities: {
      threadRead: BinaryPermission.YES,
      runUpdate: BinaryPermission.YES,
      toolsInvoke: { mode: TargetSelection.ALL },
    },
  });
  connectedFixtures.push(
    await connectAgentFixture({
      url,
      accessToken: managementToken,
      agentId: agent.id,
      options: agentOptions,
    }),
  );
  const { apiKey } = await management.tildeChat.getCredentials({ agentId: agent.id });
  assert.equal((await management.tildeChat.getCredentials({ agentId: agent.id })).apiKey, apiKey);
  // The agent's Tilde connection starts private; open it so asserted identities are admitted.
  const { routes: channelRoutes } = await management.access.listChannelAccess({
    agentId: agent.id,
  });
  const tildeRoute = channelRoutes.find((route) => route.providerId === "tilde");
  assert(tildeRoute, "every agent owns a Tilde connection");
  await management.access.setChannelAccess({
    agentId: agent.id,
    connectionId: tildeRoute.connectionId,
    mode: ChannelAccessMode.PUBLIC,
  });
  const client = (identity) =>
    createTildeChatClient({ baseUrl: url, agentId: agent.id, apiKey, identity });
  const alice = client("application:user:alice");
  const bob = client("application:user:bob");
  const { user: aliceUser } = await alice.getIdentity({});
  const { user: bobUser } = await bob.getIdentity({});
  assert.notEqual(aliceUser.id, bobUser.id);
  assert.equal((await alice.createUser({ name: "forged identity" })).user.id, aliceUser.id);
  const { thread } = await alice.createThread({
    title: "Alice private",
    primaryAgentId: agent.id,
    participants: [{ userId: bobUser.id }],
  });
  assert(thread.participants.some((p) => p.userId === aliceUser.id));
  assert(!thread.participants.some((p) => p.userId === bobUser.id));
  const { thread: bobThread } = await bob.createThread({
    title: "Bob private",
    primaryAgentId: agent.id,
  });
  assert.deepEqual(
    (await bob.listSessions({})).sessions.map((s) => s.thread.id),
    [bobThread.id],
  );
  for (const operation of [
    () => bob.getThread({ id: thread.id }),
    () => bob.listMessages({ threadId: thread.id }),
    () => bob.listQueuedMessages({ threadId: thread.id }),
    () => bob.renameSession({ threadId: thread.id, title: "stolen" }),
    () => bob.searchMessages({ threadId: thread.id, query: "first" }),
    () => bob.addParticipant({ threadId: thread.id, participant: { userId: bobUser.id } }),
  ])
    await assert.rejects(operation, (e) => e.code === Code.PermissionDenied);
  const actor = thread.participants.find((p) => p.userId === aliceUser.id).id;
  const post = async (text) =>
    alice.postMessage({ id: randomUUID(), threadId: thread.id, participantId: actor, text });
  await assert.rejects(
    alice.postMessage({
      id: randomUUID(),
      threadId: thread.id,
      participantId: bobThread.participants.find((p) => p.userId === bobUser.id).id,
      text: "forged",
    }),
    (e) => e.code === Code.PermissionDenied,
  );
  await post("first");
  await eventually(() => contexts.some((c) => c.agentId === agent.id));
  const second = (await post("second")).message;
  const third = (await post("third")).message;
  const fourth = (await post("fourth")).message;
  await eventually(
    async () => (await alice.listQueuedMessages({ threadId: thread.id })).messages.length === 3,
  );
  await alice.removeQueuedMessage({ threadId: thread.id, id: third.id });
  await alice.reorderQueuedMessage({ threadId: thread.id, id: fourth.id, beforeId: second.id });
  assert.deepEqual(
    (await alice.listQueuedMessages({ threadId: thread.id })).messages.map((m) => m.id),
    [fourth.id, second.id],
  );
  await alice.steerQueuedMessage({ threadId: thread.id, id: fourth.id });
  await eventually(() => contexts.filter((c) => c.agentId === agent.id).length === 3);
  assert.deepEqual(
    contexts.filter((c) => c.agentId === agent.id).map((c) => c.objective),
    ["first", "fourth", "second"],
  );
  await assert.rejects(
    alice.removeQueuedMessage({ threadId: thread.id, id: fourth.id }),
    (e) => e.code === Code.FailedPrecondition,
  );
  await alice.renameSession({ threadId: thread.id, title: "Renamed" });
  assert.equal((await alice.listSessions({ query: "Renamed" })).sessions[0].thread.id, thread.id);
  assert.equal((await alice.listSessions({ query: "fourth" })).sessions[0].thread.id, thread.id);
  assert.equal((await bob.listSessions({ query: "fourth" })).sessions.length, 0);
  assert(
    (await alice.searchMessages({ threadId: thread.id, query: "fourth" })).messages.some(
      (m) => m.id === fourth.id,
    ),
  );
  const upload = await alice.uploadAttachment({
    id: randomUUID(),
    threadId: thread.id,
    filename: "note.txt",
    mediaType: "text/plain",
    content: new TextEncoder().encode("attachment"),
  });
  await assert.rejects(
    bob.downloadAttachment({ threadId: thread.id, attachmentId: upload.attachment.id }),
    (e) => e.code === Code.PermissionDenied,
  );
  assert.equal(
    new TextDecoder().decode(
      (await alice.downloadAttachment({ threadId: thread.id, attachmentId: upload.attachment.id }))
        .content,
    ),
    "attachment",
  );
  await alice.addParticipant({ threadId: thread.id, participant: { userId: bobUser.id } });
  await eventually(
    async () => (await bob.listSessions({ query: "Renamed" })).sessions.length === 1,
  );
  await eventually(
    async () => (await alice.listQueuedMessages({ threadId: thread.id })).messages.length === 0,
  );
  await eventually(
    async () =>
      (await alice.getSession({ threadId: thread.id })).session.run.invocationStatus === "stopped",
  );
  const snapshot = await alice.getSession({ threadId: thread.id });
  await alice.setReadState({
    threadId: thread.id,
    throughSequence: snapshot.sequence,
    unread: false,
  });
  assert.equal((await alice.getSession({ threadId: thread.id })).session.unread, false);
  await bob.setReadState({ threadId: thread.id, unread: true });
  assert.equal((await bob.getSession({ threadId: thread.id })).session.unread, true);
  assert.equal((await alice.getSession({ threadId: thread.id })).session.unread, false);
  // Identity filtering happens before pagination, including sparse searches.
  for (let i = 0; i < 2; i++)
    await alice.createThread({ title: `Extra ${i}`, primaryAgentId: agent.id });
  const seen = [];
  let pageToken = "";
  do {
    const page = await alice.listSessions({ pageSize: 1, pageToken });
    seen.push(...page.sessions.map((s) => s.thread.id));
    pageToken = page.nextPageToken;
  } while (pageToken);
  assert.equal(seen.length, 3);
  assert(!seen.includes(bobThread.id));
  // Real binary Connect requests pass through the framework adapter to the Rust provider.
  const proxy = createChatProxy({
    apiKey,
    agentId: agent.id,
    baseUrl: url,
    resolveIdentity: (request) =>
      request.headers.get("cookie") === "session=alice" ? "application:user:alice" : null,
  });
  const routes = createChatRouteHandlers(proxy);
  const response = await routes.GET(
    new Request("http://app.test/api/chat/agents", { headers: { cookie: "session=alice" } }),
    { params: Promise.resolve({ path: ["agents"] }) },
  );
  assert.equal(response.status, 200);
  assert.equal((await response.json()).agents[0].id, agent.id);
  const proxyClient = createClient(
    ChatService,
    createConnectTransport({
      baseUrl: `http://app.test/api/chat/${agent.id}`,
      fetch: async (input, init) => {
        const request = new Request(input, init);
        request.headers.set("cookie", "session=alice");
        request.headers.set(
          "x-tilde-identity",
          Buffer.from("application:user:bob").toString("base64url"),
        );
        request.headers.set("authorization", "Bearer attacker");
        return proxy.handle(request);
      },
    }),
  );
  assert.equal((await proxyClient.getIdentity({})).user.id, aliceUser.id);
  assert.equal((await proxyClient.listSessions({})).sessions.length, 3);
  const abort = new AbortController();
  const stream = proxyClient.watchThread(
    { threadId: thread.id, afterCursor: snapshot.cursor },
    { signal: AbortSignal.any([abort.signal, AbortSignal.timeout(10000)]) },
  );
  const iterator = stream[Symbol.asyncIterator]();
  const pendingEvent = iterator.next();
  await alice.renameSession({ threadId: thread.id, title: "Stream update" });
  const first = await pendingEvent;
  assert(first.value?.cursor);
  abort.abort();
  await iterator.return?.();
  assert.equal(
    (
      await proxy.handle(
        new Request(
          `http://app.test/api/chat/${agent.id}/tilde.management.v1.AgentService/ListAgents`,
          { method: "POST", body: "{}" },
        ),
      )
    ).status,
    404,
  );
  assert.equal(
    (
      await proxy.handle(
        new Request(`http://app.test/api/chat/${agent.id}/${ChatService.typeName}/ListSessions`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: "{}",
        }),
      )
    ).status,
    401,
  );
  assert.equal(
    (
      await proxy.handle(
        new Request(`http://app.test/api/chat/${agent.id}/${ChatService.typeName}/ListSessions`, {
          method: "POST",
          headers: {
            "content-type": "application/json",
            origin: "https://evil.test",
            cookie: "session=alice",
          },
          body: "{}",
        }),
      )
    ).status,
    403,
  );
  const { agent: otherAgent } = await management.agents.createAgent({
    name: "Other agent",
    capabilities: { toolsInvoke: { mode: TargetSelection.ALL }, runUpdate: BinaryPermission.YES },
  });
  connectedFixtures.push(
    await connectAgentFixture({
      url,
      accessToken: managementToken,
      agentId: otherAgent.id,
      options: agentOptions,
    }),
  );
  await assert.rejects(
    alice.addParticipant({ threadId: thread.id, participant: { agentId: otherAgent.id } }),
    (e) => e.code === Code.PermissionDenied,
  );
  const { token: operatorToken } = await management.deployments.issueIngressToken({
    agentId: agent.id,
  });
  const operator = createTildeChatClient({
    baseUrl: url,
    agentId: agent.id,
    accessToken: operatorToken,
  });
  const shared = await operator.addParticipant({
    threadId: thread.id,
    participant: { agentId: otherAgent.id },
  });
  const target = shared.thread.participants.find((p) => p.agentId === otherAgent.id).id;
  await assert.rejects(
    alice.postMessage({
      id: randomUUID(),
      threadId: thread.id,
      participantId: actor,
      text: "unauthorized agent",
      addressedParticipantIds: [target],
    }),
    (e) => e.code === Code.PermissionDenied,
  );
  await assert.rejects(
    alice.startRun({
      threadId: thread.id,
      agentId: otherAgent.id,
      objective: "unauthorized agent",
      idempotencyKey: randomUUID(),
    }),
    (e) => e.code === Code.PermissionDenied,
  );
  const { run: foreignRun } = await operator.startRun({
    threadId: thread.id,
    agentId: otherAgent.id,
    objective: "another agent",
    idempotencyKey: randomUUID(),
  });
  await assert.rejects(
    alice.getRun({ id: foreignRun.id }),
    (e) => e.code === Code.PermissionDenied,
  );
  await assert.rejects(
    alice.cancelInvocation({ invocationId: foreignRun.invocationId }),
    (e) => e.code === Code.PermissionDenied,
  );
  const participant = shared.thread.participants.find((p) => p.userId === bobUser.id).id;
  await alice.removeParticipant({ threadId: thread.id, participantId: participant });
  await assert.rejects(
    bob.getSession({ threadId: thread.id }),
    (e) => e.code === Code.PermissionDenied,
  );
  // A suspended invocation can retain queued inputs while a newer loop is running.
  const { thread: restartThread } = await alice.createThread({
    title: "Restart queue",
    primaryAgentId: agent.id,
  });
  const restartActor = restartThread.participants.find((p) => p.userId === aliceUser.id).id;
  const restartPost = (text) =>
    alice.postMessage({
      id: randomUUID(),
      threadId: restartThread.id,
      participantId: restartActor,
      text,
    });
  await restartPost("first");
  const stopped = await eventually(() => contexts.find((ctx) => ctx.threadId === restartThread.id));
  const retained = (await restartPost("retained input")).message;
  await eventually(
    async () =>
      (await alice.listQueuedMessages({ threadId: restartThread.id })).messages.length === 1,
  );
  await alice.suspendInvocation({ invocationId: stopped.invocationId });
  await eventually(
    async () => (await alice.getRun({ id: stopped.runId })).run.invocationStatus === "stopped",
  );
  await restartPost("first");
  await eventually(() => contexts.filter((ctx) => ctx.threadId === restartThread.id).length === 2);
  await alice.steerQueuedMessage({ threadId: restartThread.id, id: retained.id });
  await eventually(() =>
    contexts.some((ctx) => ctx.threadId === restartThread.id && ctx.objective === "retained input"),
  );
  await eventually(
    async () =>
      (await alice.listQueuedMessages({ threadId: restartThread.id })).messages.length === 0,
  );
  const { apiKey: rotated } = await management.tildeChat.rotateCredentials({ agentId: agent.id });
  assert.notEqual(rotated, apiKey);
  await assert.rejects(alice.listSessions({}), (e) => e.code === Code.PermissionDenied);
  const replacement = createTildeChatClient({
    baseUrl: url,
    agentId: agent.id,
    apiKey: rotated,
    identity: "application:user:alice",
  });
  assert.equal((await replacement.getIdentity({})).user.id, aliceUser.id);
  assert(!logs.includes(apiKey));
  assert(!logs.includes(rotated));
  assert.deepEqual(errors, []);
  console.log(
    "PASS: provider key rotation, identity isolation, filtered search/pagination, attachments, per-user reads, queue dispatch and binary Next/proxy streaming",
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
  await oidc.stop();
  await rm(scratch, { recursive: true, force: true });
}
