import assert from "node:assert/strict";
import { test } from "node:test";
import { MessageList } from "@mastra/core/agent";
import { convertToMastraMessages } from "../dist/index.js";

test("converted context is accepted by Mastra's message list without a model-message conversion", async () => {
  const messages = await convertToMastraMessages({
    messages: [
      {
        type: "message",
        id: "m1",
        role: "user",
        message: { id: "m1", status: "complete", text: "Hello", attachments: [] },
      },
      {
        type: "message",
        id: "m2",
        role: "assistant",
        message: { id: "m2", status: "complete", text: "Hi there", attachments: [] },
      },
      { type: "objective", id: "o", objective: "Reply to the greeting" },
    ],
  });
  const model = new MessageList().add(messages, "input").get.all.aiV5.model();
  assert.deepEqual(
    model.map((m) => [m.role, m.content.map((part) => part.text ?? part).join("")]),
    [
      ["user", "Hello"],
      ["assistant", "Hi there"],
      ["user", "Reply to the greeting"],
    ],
  );
});
