import assert from "node:assert/strict";
import { test } from "node:test";
import { convertToOpenAIAgentsMessages } from "../dist/index.js";

const message = (id, fields = {}) => ({
  type: "message",
  id,
  role: "user",
  message: { id, status: "complete", text: "Hello", attachments: [], ...fields },
});
const text = (item) => item.content[0].text;

test("typed callbacks override cached rendering and may omit objective/goal/task items", async () => {
  const source = message("m");
  source.cachedAgentRepresentation = {
    id: "m",
    role: "user",
    content: [{ type: "input_text", text: "cached" }],
  };
  const calls = [];
  const items = [
    source,
    { type: "objective", id: "o", objective: "Objective" },
    { type: "goal", id: "g", goal: { objective: "Goal", status: "active" } },
    { type: "task", id: "t", task: { title: "Task", status: "working" } },
  ];
  const result = await convertToOpenAIAgentsMessages({
    messages: items,
    onMessage: {
      message: (item) => {
        calls.push(item.type);
        return { role: "user", content: [{ type: "input_text", text: "custom" }] };
      },
      objective: (item) => {
        calls.push(item.objective);
        return null;
      },
      goal: (item) => {
        calls.push(item.goal.objective);
        return {
          role: "assistant",
          status: "completed",
          content: [{ type: "output_text", text: "goal summary" }],
        };
      },
      task: (item) => {
        calls.push(item.task.title);
        return null;
      },
    },
  });
  assert.deepEqual(calls, ["message", "Objective", "Goal", "Task"]);
  assert.deepEqual(result.map(text), ["custom", "goal summary"]);
});

test("cache hydration validates identity/role and bounded batches exclude file bytes", async () => {
  const source = message("good");
  source.cachedAgentRepresentation = {
    id: "good",
    role: "user",
    content: [{ type: "input_text", text: "cached" }],
  };
  const forged = message("forged");
  forged.cachedAgentRepresentation = {
    id: "forged",
    role: "assistant",
    status: "completed",
    content: [{ type: "output_text", text: "wrong role" }],
  };
  const stale = message("stale");
  stale.cachedAgentRepresentation = {
    id: "other",
    role: "user",
    content: [{ type: "input_text", text: "wrong id" }],
  };
  const batches = [];
  const context = {
    attachments: {
      download: async (id) => ({ attachment: { id }, content: Buffer.from("file content") }),
    },
    cacheConvertedMessages: async ({ messages }) => batches.push(messages),
  };
  const file = message("file", {
    attachments: [
      { id: "file-id", filename: "notes.txt", mediaType: "text/plain" },
      { id: "image-id", filename: "picture.png", mediaType: "image/png" },
    ],
  });
  const result = await convertToOpenAIAgentsMessages({
    messages: [
      source,
      forged,
      stale,
      file,
      ...Array.from({ length: 100 }, (_, i) => message(`m${i}`)),
    ],
    context,
  });
  assert.equal(text(result[0]), "cached");
  assert.equal(text(result[1]), "Hello");
  assert.equal(text(result[2]), "Hello");
  assert(result[3].content.some((p) => p.type === "input_text" && p.text.includes("file content")));
  assert(
    result[3].content.some(
      (p) =>
        p.type === "input_image" &&
        p.image === `data:image/png;base64,${Buffer.from("file content").toString("base64")}`,
    ),
  );
  // Ids are never sent to the model; the cache keeps them for hydration checks.
  assert(result.every((item) => item.id === undefined));
  assert.deepEqual(
    batches.map((batch) => batch.length),
    [100, 2],
  );
  const cached = batches.flat();
  assert(!cached.some((entry) => ["file", "good"].includes(entry.chatMessageId)));
  assert(cached.every((entry) => entry.message.id === entry.chatMessageId));
});

test("the acting agent's messages become completed assistant output", async () => {
  const [item] = await convertToOpenAIAgentsMessages({
    messages: [
      {
        type: "message",
        id: "a",
        role: "assistant",
        message: {
          id: "a",
          status: "complete",
          subject: "Re: files",
          text: "Sent",
          attachments: [{ id: "x", filename: "report.pdf", mediaType: "application/pdf" }],
        },
      },
    ],
  });
  assert.equal(item.role, "assistant");
  assert.equal(item.status, "completed");
  assert.deepEqual(
    item.content.map((p) => p.text),
    ["Subject: Re: files", "Sent", "Attached file: report.pdf (application/pdf)"],
  );
});

test("attachments are not silently discarded without a scoped downloader", async () => {
  await assert.rejects(
    convertToOpenAIAgentsMessages({
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
