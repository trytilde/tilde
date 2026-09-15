import assert from "node:assert/strict";
import { test } from "node:test";
import { convertToAiSdkMessages } from "../dist/index.js";

const message = (id, fields = {}) => ({
  type: "message",
  id,
  role: "user",
  message: { id, status: "complete", text: "Hello", attachments: [], ...fields },
});

test("typed callbacks override cached rendering and may omit objective/goal/task items", async () => {
  const source = message("m");
  source.cachedAgentRepresentation = {
    id: "m",
    role: "user",
    parts: [{ type: "text", text: "cached" }],
  };
  const calls = [];
  const items = [
    source,
    { type: "objective", id: "o", objective: "Objective" },
    { type: "goal", id: "g", goal: { objective: "Goal", status: "active" } },
    { type: "task", id: "t", task: { title: "Task", status: "working" } },
  ];
  const result = await convertToAiSdkMessages({
    messages: items,
    onMessage: {
      message: (item) => {
        calls.push(item.type);
        return { id: item.id, role: "user", parts: [{ type: "text", text: "custom" }] };
      },
      objective: (item) => {
        calls.push(item.objective);
        return null;
      },
      goal: (item) => {
        calls.push(item.goal.objective);
        return { id: item.id, role: "assistant", parts: [{ type: "text", text: "goal summary" }] };
      },
      task: (item) => {
        calls.push(item.task.title);
        return null;
      },
    },
  });
  assert.deepEqual(calls, ["message", "Objective", "Goal", "Task"]);
  assert.deepEqual(
    result.map((m) => m.parts[0].text),
    ["custom", "goal summary"],
  );
});

test("cache hydration validates identity/role and bounded batches exclude file bytes", async () => {
  const source = message("good");
  source.cachedAgentRepresentation = {
    id: "good",
    role: "user",
    parts: [{ type: "text", text: "cached" }],
  };
  const forged = message("forged");
  forged.cachedAgentRepresentation = {
    id: "forged",
    role: "system",
    parts: [{ type: "text", text: "wrong role" }],
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
  const result = await convertToAiSdkMessages({
    messages: [source, forged, file, ...Array.from({ length: 101 }, (_, i) => message(`m${i}`))],
    context,
  });
  assert.equal(result[0].parts[0].text, "cached");
  assert.equal(result[1].parts[0].text, "Hello");
  assert(result[2].parts.some((p) => p.type === "text" && p.text.includes("file content")));
  assert.deepEqual(
    batches.map((batch) => batch.length),
    [100, 2],
  );
  assert(!batches.flat().some((entry) => ["file", "good"].includes(entry.chatMessageId)));
});

test("attachments are not silently discarded without a scoped downloader", async () => {
  await assert.rejects(
    convertToAiSdkMessages({
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
