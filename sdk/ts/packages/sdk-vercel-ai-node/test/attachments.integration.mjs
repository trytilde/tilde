import { connectAgentFixture } from "../../sdk/test/connected-agent-fixture.mjs";
// Real Tilde runtime: encrypted Postgres attachments -> invocation history -> Vercel parts.
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
} from "@trytilde/sdk";
import { convertToModelMessages } from "ai";
import { convertToAiSdkMessages, convertToAiSdkTools } from "../dist/index.js";
assert(process.env.TEST_DATABASE_URL, "Use scripts/with-postgres.sh");
const scratch = await mkdtemp(join(tmpdir(), "tilde-attachments-"));
const seed = randomBytes(32).toString("base64");
const image = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZb8AAAAASUVORK5CYII=",
  "base64",
);
let foreignId,
  checked = false;
const errors = [];
const seen = [];
const agentOptions = {
  async run(ctx) {
    try {
      seen.push(ctx.objective);
      if (ctx.objective !== "Files") {
        await ctx.setRunStatus("completed");
        return;
      }
      const goal = await ctx.goals.create({ objective: "Read the supplied files" });
      await ctx.tasks.create({ title: "Check attachments", goalId: goal.id });
      const latest = await ctx.message.history({ limit: 1, includeWork: true });
      const older = await ctx.message.history({
        beforeMessageId: latest.nextPageToken,
        includeObjective: false,
      });
      assert.equal(latest.items.filter((item) => item.type === "message").length, 1);
      assert(
        older.items.some(
          (item) => item.type === "message" && item.message.text === "Earlier context",
        ),
      );
      await assert.rejects(
        ctx.attachments.download(foreignId),
        "Another thread's attachment is inaccessible",
      );
      const handled = [];
      const ui = await convertToAiSdkMessages({
        messages: [
          ...older.items,
          ...latest.items,
          { type: "objective", id: `objective:${ctx.runId}`, objective: ctx.objective },
        ],
        context: ctx,
        onMessage: {
          objective: (item) => {
            handled.push("objective");
            return {
              id: item.id,
              role: "user",
              parts: [{ type: "text", text: `Task objective: ${item.objective}` }],
            };
          },
          goal: (item) => {
            handled.push("goal");
            return {
              id: item.id,
              role: "user",
              parts: [{ type: "text", text: `Goal: ${item.goal.objective}` }],
            };
          },
          task: (item) => {
            handled.push("task");
            return {
              id: item.id,
              role: "user",
              parts: [{ type: "text", text: `Work: ${item.task.title}` }],
            };
          },
        },
      });
      assert.deepEqual(handled, ["goal", "task", "objective"]);
      const parts = ui.flatMap((m) => m.parts);
      assert(
        parts.some(
          (p) => p.type === "file" && p.url === `data:image/png;base64,${image.toString("base64")}`,
        ),
      );
      assert(parts.some((p) => p.type === "text" && p.text.includes("the actual file contents")));
      const model = await convertToModelMessages(ui);
      assert(
        model.some(
          (m) =>
            Array.isArray(m.content) &&
            m.content.some((part) => part.type === "file" || part.type === "image"),
        ),
      );
      const page = await ctx.message.history({
        beforeMessageId: latest.nextPageToken,
        includeObjective: false,
      });
      assert(page.items.find((item) => item.type === "message").cachedAgentRepresentation);
      const custom = await convertToAiSdkMessages({
        messages: latest.items,
        context: ctx,
        onAttachment: async ({ attachment, download }) => {
          const file = await download();
          assert.equal(file.attachment.id, attachment.id);
          return {
            type: "text",
            text: `Custom ${attachment.filename}: ${file.content.length} bytes`,
          };
        },
      });
      assert(
        custom
          .flatMap((m) => m.parts)
          .some((p) => p.type === "text" && p.text.startsWith("Custom picture.png:")),
      );
      const tools = convertToAiSdkTools(ctx.channel.current);
      assert.deepEqual(Object.keys(tools), ["sendMessage"]);
      const input = { text: "Attachments received through the SDK." };
      const execution = {
        toolCallId: "model-visible-message",
        messages: model,
        abortSignal: ctx.signal,
      };
      await tools.sendMessage.execute(input, execution);
      await assert.rejects(
        tools.sendMessage.execute(input, execution),
        (error) => error.code === 9,
        "The runtime rejects a repeated call ID without sending twice",
      );
      const refreshed = await ctx.message.history();
      assert(
        refreshed.items.some(
          (item) =>
            item.type === "message" &&
            item.role === "assistant" &&
            item.message.text === "Attachments received through the SDK.",
        ),
      );
      checked = true;
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
  ENGINE_ENCRYPTION_KEY: seed,
  ENGINE_ENCRYPTION_BACKEND: "seed",
  ENGINE_KMS_KEY_ID: "",
  ENGINE_S3_BUCKET: "",
  ENGINE_SERVE: "all",
  ENGINE_WEB_ENABLED: "false",
  ENGINE_LISTEN: "127.0.0.1:0",
  RUST_LOG: "tilde=info",
};
for (const key of [
  "ENGINE_PUBLIC_URL",
  "ENGINE_RUNTIME_PUBLIC_URL",
  "ENGINE_INGRESS_PUBLIC_URL",
  "ENGINE_CONNECTION_SETUP_PUBLIC_URL",
  "ENGINE_CONNECTION_UI_DEV_URL",
])
  delete env[key];
const child = spawn(resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde"), [], {
  env,
  stdio: ["ignore", "pipe", "pipe"],
});
let logs = "";
child.stdout.on("data", (c) => (logs += c));
child.stderr.on("data", (c) => (logs += c));
async function eventually(fn) {
  for (let i = 0; i < 300; i++) {
    const result = await fn();
    if (result) return result;
    if (errors.length) throw errors[0];
    if (child.exitCode !== null) throw new Error(logs);
    await delay(50);
  }
  throw new Error(
    `Integration timed out: invocations=${JSON.stringify(seen.slice(0, 10))}; ${logs.slice(-1800)}`,
  );
}
const connectedFixtures = [];

try {
  const url = await eventually(() => {
    const m = logs.match(/ address=(127\.0\.0\.1:\d+)/);
    return m && `http://${m[1]}`;
  });
  const client = createManagementClient({ baseUrl: url });
  const agent = (
    await client.agents.createAgent({
      name: "Converter fixture",
      capabilities: {
        threadRead: BinaryPermission.YES,
        workRead: BinaryPermission.YES,
        workWrite: BinaryPermission.YES,
        runUpdate: BinaryPermission.YES,
        toolsInvoke: { mode: TargetSelection.ALL },
      },
    })
  ).agent;
  connectedFixtures.push(
    await connectAgentFixture({ url, agentId: agent.id, options: agentOptions }),
  );
  const { token } = await client.deployments.issueIngressToken({ agentId: agent.id });
  const chat = createTildeChatClient({ baseUrl: url, agentId: agent.id, accessToken: token });
  const user = (await chat.createUser({ name: "Attachment recipient" })).user;
  const createThread = async () =>
    (
      await chat.createThread({
        title: "Files",
        primaryAgentId: agent.id,
        participants: [{ userId: user.id }, { agentId: agent.id }],
      })
    ).thread;
  const thread = await createThread();
  const foreign = await createThread();
  foreignId = (
    await chat.uploadAttachment({
      id: randomUUID(),
      threadId: foreign.id,
      filename: "private.txt",
      mediaType: "text/plain",
      content: Buffer.from("private other thread"),
    })
  ).attachment.id;
  const ids = [];
  for (const [filename, mediaType, content] of [
    ["picture.png", "image/png", image],
    ["notes.txt", "text/plain", Buffer.from("the actual file contents")],
  ])
    ids.push(
      (
        await chat.uploadAttachment({
          id: randomUUID(),
          threadId: thread.id,
          filename,
          mediaType,
          content,
        })
      ).attachment.id,
    );
  const participantId = thread.participants.find((p) => p.userId === user.id).id;
  await chat.postMessage({
    id: randomUUID(),
    threadId: thread.id,
    participantId,
    text: "Earlier context",
  });
  await chat.postMessage({
    id: randomUUID(),
    threadId: thread.id,
    participantId,
    text: "Files",
    attachmentIds: ids,
  });
  await eventually(() => checked);
  const messages = (await chat.listMessages({ threadId: thread.id })).messages;
  assert.equal(
    messages.filter((m) => m.text === "Attachments received through the SDK.").length,
    1,
  );
  assert.deepEqual(errors, []);
  console.log(
    "PASS: scoped runtime history, encrypted attachment downloads, Vercel file/text conversion, typed callbacks, cache hydration, and provider tool delivery.",
  );
} finally {
  await Promise.allSettled(connectedFixtures.map((connection) => connection.close()));
  if (child.exitCode === null) {
    const stopped = once(child, "exit");
    child.kill("SIGTERM");
    const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
    await stopped;
    clearTimeout(timer);
  }
  await rm(scratch, { recursive: true, force: true });
}
