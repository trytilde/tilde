import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { test } from "node:test";
import { runWithContext } from "@trytilde/sdk";
import { createSkillsClient } from "../../../packages/sdk/dist/skills.js";
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
  const signal = new AbortController().signal;
  const published = [];
  const audits = [];
  return {
    sent,
    published,
    audits,
    signal,
    inference: () => ({
      baseURL,
      apiKey: "tilde",
      fetch: (input, init) => {
        const headers = new Headers(init?.headers);
        headers.set("authorization", "Bearer fixture-key");
        return send(input, { ...init, headers });
      },
    }),
    activatePrompt: () => {},
    // Steering arrives while the model works: handed over at the second step.
    takeInputs: () => (sent.length ? steering.splice(0) : []),
    // A skill assigned in the registry: the AI SDK has no native skills, so it reaches the
    // model as Tilde's skill tools and a summary in the instructions.
    skills: createSkillsClient(
      {
        listSkills: async () => ({
          skills: [
            {
              name: "refunds",
              source: "support",
              description: "Handle refunds",
              versionId: "v1",
              deployed: false,
              files: [{ path: "SKILL.md" }],
            },
          ],
        }),
        readSkillFile: async () => ({ content: "# Refunds", mediaType: "text/markdown" }),
      },
      () => ({ headers: new Headers(), signal }),
    ),
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
    assert.deepEqual(
      requests[0].tools.map((tool) => tool.name),
      ["sendMessage", "local_time", "roll_dice", "list_skills", "read_skill"],
    );
    const system = requests[0].input[0];
    // Reasoning models take the instructions as a developer message.
    assert.equal(system.role, "developer");
    assert.match(system.content, /^You are Example Agent 1/);
    assert.match(system.content, /- refunds: Handle refunds$/);
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
    assert(
      requests[1].input.some(
        (item) => item.type === "function_call_output" && item.call_id === "call_send",
      ),
    );
    assert.equal(
      requests[1].input.at(-1).content[0].text,
      "Also mention the weather",
      "steering input reaches the model as a user message at the next step",
    );
    assert.equal(requests[0].store, false);
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
