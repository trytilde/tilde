import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { test } from "node:test";
import { createOpenAI } from "@ai-sdk/openai";
import { respond } from "../dist/agent.js";

function context() {
  const sent = [];
  const sendMessage = {
    description: "Send a visible message to this conversation.",
    inputSchema: {
      type: "object",
      properties: { text: { type: "string" } },
      required: ["text"],
      additionalProperties: false,
    },
    execute: async (input, execution) => {
      sent.push({ input, execution });
      return { accepted: true };
    },
  };
  return {
    sent,
    signal: new AbortController().signal,
    channel: { current: { sendMessage } },
    message: {
      history: async () => ({
        items: [
          {
            type: "message",
            id: "received",
            role: "user",
            message: { id: "received", text: "Hi", status: "complete", attachments: [] },
          },
        ],
      }),
    },
    cacheConvertedMessages: async () => {},
  };
}
function response(output) {
  return { id: "resp_fixture", created_at: 1700000000, model: "gpt-4o-mini", output };
}
const privateText = {
  id: "msg_private",
  type: "message",
  role: "assistant",
  content: [{ type: "output_text", text: "Private model completion", annotations: [] }],
};

test("the model uses current-channel tools; returned model text is never sent", async () => {
  const requests = [];
  const server = createServer(async (request, res) => {
    let body = "";
    for await (const chunk of request) body += chunk;
    requests.push(JSON.parse(body));
    assert.equal(request.headers.authorization, "Bearer fixture-key");
    res.setHeader("content-type", "application/json");
    res.end(
      JSON.stringify(
        response(
          requests.length === 1
            ? [
                {
                  id: "fc_send",
                  type: "function_call",
                  call_id: "call_send",
                  name: "sendMessage",
                  arguments: JSON.stringify({ text: "Hello through the provider tool" }),
                },
              ]
            : [privateText],
        ),
      ),
    );
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  try {
    const ctx = context();
    const model = createOpenAI({
      apiKey: "fixture-key",
      baseURL: `http://127.0.0.1:${server.address().port}`,
    }).responses("gpt-4o-mini");
    await respond(ctx, model);
    assert.deepEqual(ctx.sent, [
      {
        input: { text: "Hello through the provider tool" },
        execution: { toolCallId: "call_send" },
      },
    ]);
    assert.deepEqual(
      requests[0].tools.map((tool) => tool.name),
      ["sendMessage"],
    );
    assert(
      requests[1].input.some(
        (item) => item.type === "function_call_output" && item.call_id === "call_send",
      ),
    );
    assert.equal(requests[0].store, false);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});

test("text-only completion and upstream failures do not produce implicit messages", async () => {
  const ctx = context();
  await respond(
    ctx,
    createOpenAI({
      apiKey: "fixture",
      fetch: async () =>
        new Response(JSON.stringify(response([privateText])), {
          headers: { "content-type": "application/json" },
        }),
    }).responses("gpt-4o-mini"),
  );
  assert.deepEqual(ctx.sent, []);
  await assert.rejects(
    respond(
      ctx,
      createOpenAI({
        apiKey: "private-key",
        fetch: async () => new Response("private upstream detail", { status: 401 }),
      }).responses("gpt-4o-mini"),
    ),
    /^Error: OpenAI inference failed \(401\)$/,
  );
  assert.deepEqual(ctx.sent, []);
});
