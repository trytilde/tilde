import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { test } from "node:test";
import { runWithContext } from "@trytilde/sdk";
import { respond } from "../dist/agent.js";

/** The module-scope agent's model resolves `ctx.inference` of the running invocation. */
function context({ baseURL = "http://127.0.0.1:1", fetch: send = fetch, steering = [] } = {}) {
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
  const readSkill = {
    description: "Read a skill file.",
    inputSchema: { type: "object", properties: { name: { type: "string" } }, required: ["name"] },
    execute: async () => ({ content: "Skill body" }),
  };
  const stamps = [];
  const published = [];
  const audits = [];
  return {
    sent,
    stamps,
    published,
    audits,
    signal: new AbortController().signal,
    inference: () => ({
      baseURL,
      apiKey: "tilde",
      fetch: (input, init) => {
        const headers = new Headers(init?.headers);
        headers.set("authorization", "Bearer fixture-key");
        return send(input, { ...init, headers });
      },
    }),
    activatePrompt: (prompt) => stamps.push(prompt),
    // Steering arrives while the model works: handed over at the second model call.
    takeInputs: () => (sent.length ? steering.splice(0) : []),
    channel: { current: { sendMessage } },
    agentTools: {},
    setBundledTools: async (tools) => void published.push(...tools.map((tool) => tool.name)),
    reportToolCall: async (report) => void audits.push(report),
    // A skill assigned in the registry; this agent has no shell tool, so it arrives as tools.
    skills: {
      list: async () => [
        { name: "faq", source: "team", description: "Answer FAQs", deployed: false },
      ],
      summary: async () => "You have these skills.\n- faq: Answer FAQs",
      tools: () => ({ read_skill: readSkill }),
    },
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
  return {
    id: "resp_fixture",
    object: "response",
    created_at: 1700000000,
    status: "completed",
    model: "gpt-4o-mini",
    output,
    usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 },
  };
}
const privateText = {
  id: "msg_private",
  type: "message",
  role: "assistant",
  status: "completed",
  content: [{ type: "output_text", text: "Private model completion", annotations: [] }],
};

test("OpenAI Agents runs current-channel tools with the model's call ID; steering reaches the model", async () => {
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
                  status: "completed",
                  call_id: "call_send",
                  name: "sendMessage",
                  arguments: JSON.stringify({ text: "Hello through the provider tool" }),
                },
                {
                  id: "fc_time",
                  type: "function_call",
                  status: "completed",
                  call_id: "call_time",
                  name: "local_time",
                  arguments: JSON.stringify({ timeZone: "UTC" }),
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
    const ctx = context({
      baseURL: `http://127.0.0.1:${server.address().port}`,
      steering: [{ id: "input-1", text: "Also mention the weather" }],
    });
    await runWithContext(ctx, () => respond(ctx));
    assert.deepEqual(
      ctx.sent.map(({ input, execution }) => [input, execution.toolCallId]),
      [[{ text: "Hello through the provider tool" }, "call_send"]],
    );
    assert.deepEqual(
      requests[0].tools.map((tool) => tool.name),
      ["sendMessage", "local_time", "roll_dice", "read_skill"],
    );
    // The agent's bundled tools are published to Tilde and their calls audited with the model's ID.
    assert.deepEqual(ctx.published, ["local_time", "roll_dice"]);
    assert.deepEqual(
      ctx.audits.map((a) => [a.toolCallId, a.name, a.status, a.summary]),
      [
        ["call_time", "local_time", "running", "Checked the time"],
        ["call_time", "local_time", "completed", "Checked the time"],
      ],
    );
    assert.deepEqual(requests[0].tools[0].parameters, ctx.channel.current.sendMessage.inputSchema);
    assert(requests[0].instructions.endsWith("You have these skills.\n- faq: Answer FAQs"));
    assert.equal(requests[0].store, false);
    assert(
      requests[1].input.some(
        (item) => item.type === "function_call_output" && item.call_id === "call_send",
      ),
    );
    assert(
      requests[1].input.some(
        (item) =>
          item.role === "user" && JSON.stringify(item.content).includes("Also mention the weather"),
      ),
      "steering input reaches the model as a user message at the next call",
    );
    // Static instructions are matched by the gateway itself; nothing is stamped.
    assert.deepEqual(ctx.stamps, []);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});

test("text-only completion and upstream failures do not produce implicit messages", async () => {
  const json = (output) =>
    new Response(JSON.stringify(response(output)), {
      headers: { "content-type": "application/json" },
    });
  const ctx = context({ fetch: async () => json([privateText]) });
  await runWithContext(ctx, () => respond(ctx));
  assert.deepEqual(ctx.sent, []);
  const failing = context({
    fetch: async () => new Response("private upstream detail", { status: 401 }),
  });
  await assert.rejects(
    runWithContext(failing, () => respond(failing)),
    /^Error: OpenAI inference failed \(401\)$/,
  );
  assert.deepEqual(failing.sent, []);
});

test("outside an invocation the module-scope model refuses to call out", async () => {
  const ctx = context();
  await assert.rejects(respond(ctx), /^Error: OpenAI inference failed$/);
});
