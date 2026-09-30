import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { test } from "node:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runWithContext } from "@trytilde/sdk";
import { respond } from "../dist/agent.js";

// A skill assigned in Tilde's registry, as `ctx.skills.directory()` writes it.
const registry = mkdtempSync(join(tmpdir(), "tilde-mastra-registry-"));
mkdirSync(join(registry, "refunds"));
writeFileSync(
  join(registry, "refunds", "SKILL.md"),
  "---\nname: refunds\ndescription: Handle refund requests\n---\n\nCheck the order first.\n",
);

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
    // Steering arrives while the model works: handed over at the second step.
    takeInputs: () => (sent.length ? steering.splice(0) : []),
    skills: { directory: async () => registry },
    channel: { current: { sendMessage } },
    agentTools: {},
    setBundledTools: async (tools) => void published.push(...tools.map((tool) => tool.name)),
    reportToolCall: async (report) => void audits.push(report),
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

test("Mastra runs current-channel tools with the model's call ID; returned text is never sent", async () => {
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
                {
                  id: "fc_time",
                  type: "function_call",
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
    // The Tilde toolset (channel and bundled tools), plus Mastra's tools for the skills folder.
    assert.deepEqual(
      requests[0].tools.map((tool) => tool.name),
      ["sendMessage", "local_time", "roll_dice", "skill", "skill_search", "skill_read"],
    );
    // The agent's own tools are published to Tilde and their calls audited with the model's ID.
    assert.deepEqual(ctx.published, ["local_time", "roll_dice"]);
    assert.deepEqual(
      ctx.audits.map((a) => [a.toolCallId, a.name, a.status, a.summary]),
      [
        ["call_time", "local_time", "running", "Checked the time"],
        ["call_time", "local_time", "completed", "Checked the time"],
      ],
    );
    assert.equal(ctx.audits[1].output.timeZone, "UTC");
    // Mastra publishes the provider schema unchanged apart from a draft-07 `$schema` marker.
    const parameters = requests[0].tools[0].parameters;
    delete parameters.$schema;
    assert.deepEqual(parameters, ctx.channel.current.sendMessage.inputSchema);
    assert(
      requests[1].input.some(
        (item) => item.type === "function_call_output" && item.call_id === "call_send",
      ),
    );
    assert.equal(requests[0].store, false);
    assert(
      requests[1].input.some(
        (item) =>
          item.role === "user" && JSON.stringify(item.content).includes("Also mention the weather"),
      ),
      "steering input reaches the model as a user message at the next step",
    );
    // The deployed skill and the registry skill both reach the model through Mastra's skills.
    const system = JSON.stringify(requests[0].input.filter((item) => item.role === "system"));
    assert.match(system, /channel-replies/);
    assert.match(system, /refunds/);
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
