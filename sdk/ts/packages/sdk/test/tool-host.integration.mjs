import { connectAgentFixture } from "./connected-agent-fixture.mjs";
// A TypeScript tool host dials in; an SDK agent calls its tool through one of its tool sources.
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
import * as z from "zod";
import {
  createToolHost,
  createToolLambdaHandler,
  defineAuth,
  defineTool,
} from "../dist/tool-host.js";
assert(process.env.TEST_DATABASE_URL, "Use scripts/with-postgres.sh");
const scratch = await mkdtemp(join(tmpdir(), "tilde-tool-host-"));
const errors = [];
const calls = [];
// The slug the server derives for the host when the agent adds it as a source.
let sourceSlug;
const tools = {
  lookup_order: defineTool({
    description: "Look up an order by its number.",
    summary: "Looked up an order",
    inputSchema: z.object({ number: z.string() }),
    outputSchema: z.object({ number: z.string(), status: z.enum(["shipped", "pending"]) }),
    annotations: { readOnly: true },
    async run(input, ctx) {
      calls.push({ input, ctx });
      if (input.number === "missing") throw new Error("No such order");
      return { number: input.number, status: "shipped" };
    },
  }),
};
// The agent dials in like a real host; the gateway never calls it.
const agentOptions = {
  async run(ctx) {
    try {
      await ctx.refreshTools();
      const orders = ctx.toolSource(sourceSlug);
      assert.deepEqual(Object.keys(orders), ["lookup_order"]);
      assert.equal(orders.lookup_order.summary, "Looked up an order");
      assert.equal(orders.lookup_order.annotations.readOnly, true);
      const found = await orders.lookup_order.execute(
        { number: ctx.objective },
        { toolCallId: "a" },
      );
      await assert.rejects(
        orders.lookup_order.execute({ number: "missing" }, { toolCallId: "b" }),
        /No such order/,
      );
      await ctx.channel.current.sendMessage({ text: `order ${found.number} is ${found.status}` });
      await ctx.setRunStatus("completed");
    } catch (error) {
      errors.push(error);
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
  throw new Error("Tool host test timed out");
}
let agentConnection;
let toolHost;
try {
  // The Lambda adapter shares the execution path; it needs no engine to check its contract.
  const lambda = createToolLambdaHandler({ tools });
  assert.equal((await lambda({ listTools: {} })).tools[0].name, "lookup_order");
  // With auth, the definition is Protobuf JSON and each event carries the instance's key.
  const auth = defineAuth({
    provider: { id: "crm", name: "CRM" },
    methods: { api_key: { name: "API key", schema: z.object({ api_key: z.string() }) } },
    async verify(values) {
      if (values.api_key !== "good") throw new Error("Unknown key");
      return { accountLabel: "acme" };
    },
  });
  const crm = createToolLambdaHandler({
    auth,
    tools: {
      whoami: auth.tool({
        description: "Name the key in use.",
        inputSchema: z.object({}),
        outputSchema: z.object({ key: z.string(), instance: z.string() }),
        async run(_input, ctx) {
          return ctx.auth.api_key === "bad-output"
            ? { key: 1 }
            : { key: ctx.auth.api_key, instance: ctx.connectionId };
        },
      }),
    },
  });
  const listed = await crm({ listTools: {} });
  assert.equal(listed.provider.connectionTypes[0].static.schemaJson.includes("api_key"), true);
  const credentials = [{ key: "api_key", value: "good" }];
  assert.deepEqual(
    await crm({
      verify: { callId: "v", connectionId: "i", connectionType: "api_key", credentials },
    }),
    {
      accountLabel: "acme",
    },
  );
  assert.deepEqual(
    await crm({
      verify: {
        callId: "v",
        connectionId: "i",
        connectionType: "api_key",
        credentials: [{ key: "api_key", value: "bad" }],
      },
    }),
    { error: "Unknown key" },
  );
  assert.deepEqual(
    await crm({
      call: {
        callId: "c",
        name: "whoami",
        inputJson: "{}",
        agentId: "",
        threadId: "",
        connectionId: "i",
        connectionType: "api_key",
        credentials,
      },
    }),
    { outputJson: '{"key":"good","instance":"i"}' },
  );
  // A result outside the output schema, or input outside the input schema, is the tool's error.
  const invalid = await crm({
    call: {
      callId: "c",
      name: "whoami",
      inputJson: "{}",
      agentId: "",
      threadId: "",
      connectionId: "i",
      connectionType: "api_key",
      credentials: [{ key: "api_key", value: "bad-output" }],
    },
  });
  assert.match(invalid.error, /key/);
  const rejected = await lambda({
    call: {
      callId: "c",
      name: "lookup_order",
      inputJson: '{"number":7}',
      agentId: "",
      threadId: "",
    },
  });
  assert.match(rejected.error, /number/);
  assert.deepEqual(
    await lambda({
      call: {
        callId: "c",
        name: "lookup_order",
        inputJson: '{"number":"7"}',
        agentId: "",
        threadId: "",
      },
    }),
    { outputJson: '{"number":"7","status":"shipped"}' },
  );

  const url = await eventually(() => {
    const match = logs.match(/ address=(127\.0\.0\.1:\d+)/);
    return match && `http://${match[1]}`;
  });
  const client = createManagementClient({ baseUrl: url });
  const registered = await client.toolHosts.registerToolHost({ name: "orders-host", type: 1 });
  assert(registered.token, "connected hosts receive their token once");
  toolHost = createToolHost({ gatewayUrl: url, token: registered.token, tools });
  const hostId = registered.toolHost.id;
  await eventually(async () =>
    (await client.toolHosts.listToolHosts({})).toolHosts.find(
      (h) => h.id === hostId && h.available && h.tools.length === 1,
    ),
  );
  const agent = (
    await client.agents.createAgent({
      name: "Orders agent",
      capabilities: {
        threadRead: BinaryPermission.YES,
        runUpdate: BinaryPermission.YES,
        toolsInvoke: { mode: TargetSelection.ALL },
      },
    })
  ).agent;
  const { source } = await client.tools.addToolSource({
    agentId: agent.id,
    toolHostId: hostId,
    toolNames: ["lookup_order"],
  });
  sourceSlug = source.slug;
  agentConnection = await connectAgentFixture({
    url,
    agentId: agent.id,
    options: agentOptions,
  });
  const { token } = await client.deployments.issueIngressToken({ agentId: agent.id });
  const chat = createTildeChatClient({ baseUrl: url, agentId: agent.id, accessToken: token });
  const user = (await chat.createUser({ name: "Customer" })).user;
  const thread = (
    await chat.createThread({
      title: "Orders",
      primaryAgentId: agent.id,
      participants: [{ userId: user.id }, { agentId: agent.id }],
    })
  ).thread;
  const participantId = thread.participants.find((p) => p.userId === user.id).id;
  await chat.postMessage({ id: randomUUID(), threadId: thread.id, participantId, text: "1042" });
  await eventually(async () =>
    (await chat.listMessages({ threadId: thread.id })).messages.some(
      (m) => m.text === "order 1042 is shipped",
    ),
  );
  const served = calls.find((c) => c.input.number === "1042");
  assert.equal(served.ctx.agentId, agent.id);
  assert.equal(served.ctx.threadId, thread.id);
  assert.deepEqual(errors, []);
  console.log(
    "PASS: a dialed-in TypeScript tool host serves an SDK agent through a tool source; Lambda adapter contract holds.",
  );
} finally {
  toolHost?.close();
  await agentConnection?.close();
  if (child.exitCode === null) {
    const done = once(child, "exit");
    child.kill("SIGTERM");
    const timeout = setTimeout(() => child.kill("SIGKILL"), 5000);
    await done;
    clearTimeout(timeout);
  }
  await rm(scratch, { recursive: true, force: true });
}
