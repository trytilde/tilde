import assert from "node:assert/strict";
import { test } from "node:test";
import { createMessageClient } from "../dist/messages.js";

const incoming = (id, destination = "current") => ({
  id,
  participantId: "user",
  status: "complete",
  text: "Hi",
  attachments: [],
  delivery: { connectionId: "1234-abcd", externalMessageId: id, destination },
});
function context(messages) {
  return {
    agentId: "agent",
    runId: "run",
    objective: "Reply",
    participants: [
      { id: "assistant", agentId: "agent" },
      { id: "user" },
      { id: "peer", agentId: "other" },
    ],
    messages,
    getMessages: async () => ({ messages, cachedMessages: [], nextPageToken: "" }),
    goals: { list: async () => [] },
    tasks: { list: async () => [] },
    tools: {},
    sendNativeMessage: async () => {},
  };
}

test("history owns roles, pagination and typed current work", async () => {
  const ctx = context([incoming("new")]);
  const calls = [];
  ctx.getMessages = async (options) => {
    calls.push(options);
    return options.beforeMessageId
      ? { messages: [incoming("old", "older")], cachedMessages: [], nextPageToken: "" }
      : {
          messages: [
            { ...incoming("own"), participantId: "assistant" },
            { ...incoming("peer"), participantId: "peer" },
            incoming("new"),
          ],
          cachedMessages: [
            {
              messageId: "new",
              messageJson: JSON.stringify({
                id: "new",
                role: "user",
                parts: [{ type: "text", text: "converted" }],
              }),
            },
          ],
          nextPageToken: "own",
        };
  };
  ctx.goals.list = async () => [{ id: "g", objective: "Goal", status: "active" }];
  ctx.tasks.list = async () => [{ id: "t", title: "Task", status: "working" }];
  const client = createMessageClient(ctx);
  const history = await client.history({ limit: 3, includeWork: true });
  assert.deepEqual(
    history.items.map((item) => item.type),
    ["message", "message", "message", "objective", "goal", "task"],
  );
  assert.deepEqual(
    history.items.slice(0, 3).map((item) => item.role),
    ["assistant", "user", "user"],
  );
  assert.equal(history.items[2].cachedAgentRepresentation.parts[0].text, "converted");
  assert.equal(history.nextPageToken, "own");
  const older = await client.history({ beforeMessageId: history.nextPageToken });
  assert.equal(older.items.length, 1, "Older pages do not duplicate the current objective");
  assert.deepEqual(calls[0], { limit: 3, beforeMessageId: undefined });
  assert.deepEqual(calls[1], { limit: undefined, beforeMessageId: "own" });
});
