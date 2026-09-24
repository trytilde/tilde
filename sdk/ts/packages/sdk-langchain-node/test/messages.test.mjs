import assert from "node:assert/strict";
import { test } from "node:test";
import { AIMessage, HumanMessage } from "@langchain/core/messages";
import { convertToLangChainMessages } from "../dist/index.js";

const message = (id, fields = {}) => ({
  type: "message",
  id,
  role: "user",
  message: { id, status: "complete", text: "Hello", attachments: [], ...fields },
});

test("typed callbacks override cached rendering and may omit objective/goal/task items", async () => {
  const source = message("m");
  source.cachedAgentRepresentation = { id: "m", role: "user", content: "cached" };
  const calls = [];
  const items = [
    source,
    { type: "objective", id: "o", objective: "Objective" },
    { type: "goal", id: "g", goal: { objective: "Goal", status: "active" } },
    { type: "task", id: "t", task: { title: "Task", status: "working" } },
  ];
  const result = await convertToLangChainMessages({
    messages: items,
    onMessage: {
      message: (item) => {
        calls.push(item.type);
        return new HumanMessage({ id: item.id, content: "custom" });
      },
      objective: (item) => {
        calls.push(item.objective);
        return null;
      },
      goal: (item) => {
        calls.push(item.goal.objective);
        return new AIMessage({ id: item.id, content: "goal summary" });
      },
      task: (item) => {
        calls.push(item.task.title);
        return null;
      },
    },
  });
  assert.deepEqual(calls, ["message", "Objective", "Goal", "Task"]);
  assert.deepEqual(
    result.map((m) => [m.id, m.getType(), m.content]),
    [
      ["m", "human", "custom"],
      ["g", "ai", "goal summary"],
    ],
  );
});

test("cache hydration validates identity/role and bounded batches exclude file bytes", async () => {
  const source = message("good");
  source.cachedAgentRepresentation = { id: "good", role: "user", content: "cached" };
  const forged = message("forged");
  forged.cachedAgentRepresentation = { id: "forged", role: "assistant", content: "wrong role" };
  const binary = message("binary");
  binary.cachedAgentRepresentation = {
    id: "binary",
    role: "user",
    content: [{ type: "image", mimeType: "image/png", data: "AAAA" }],
  };
  const batches = [];
  const context = {
    attachments: {
      download: async (id) => ({ attachment: { id }, content: Buffer.from("file content") }),
    },
    cacheConvertedMessages: async ({ messages }) => batches.push(messages),
  };
  const file = message("file", {
    attachments: [{ id: "file-id", filename: "notes.txt", mediaType: "text/plain" }],
  });
  const image = message("image", {
    attachments: [{ id: "image-id", filename: "picture.png", mediaType: "image/png" }],
  });
  const result = await convertToLangChainMessages({
    messages: [
      source,
      forged,
      binary,
      file,
      image,
      ...Array.from({ length: 101 }, (_, i) => message(`m${i}`)),
    ],
    context,
  });
  assert.equal(result[0].content, "cached");
  assert.equal(result[1].content, "Hello");
  assert.equal(result[2].content, "Hello");
  assert.deepEqual(result[3].content, [
    { type: "text", text: "Hello" },
    { type: "text", text: "Attached file: notes.txt\nfile content" },
  ]);
  assert.deepEqual(result[4].content, [
    { type: "text", text: "Hello" },
    {
      type: "image",
      mimeType: "image/png",
      data: Buffer.from("file content").toString("base64"),
    },
  ]);
  assert.deepEqual(
    result.map((m) => m.id),
    [
      "good",
      "forged",
      "binary",
      "file",
      "image",
      ...Array.from({ length: 101 }, (_, i) => `m${i}`),
    ],
  );
  assert(result.every((m) => m instanceof HumanMessage));
  assert.deepEqual(
    batches.map((batch) => batch.length),
    [100, 3],
  );
  const cached = batches.flat();
  assert(!cached.some((entry) => ["file", "image", "good"].includes(entry.chatMessageId)));
  assert.deepEqual(cached.find((entry) => entry.chatMessageId === "forged").message, {
    id: "forged",
    role: "user",
    content: "Hello",
  });
});

test("attachments are not silently discarded without a scoped downloader", async () => {
  await assert.rejects(
    convertToLangChainMessages({
      messages: [
        message("m", {
          text: "",
          attachments: [{ id: "image", filename: "picture.png", mediaType: "image/png" }],
        }),
      ],
    }),
    /requires context/,
  );
});
