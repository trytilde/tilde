import assert from "node:assert/strict";
import { test } from "node:test";
import { createChannels } from "../dist/channels.js";

const one = "11111111-1111-4111-8111-111111111111",
  two = "22222222-2222-4222-8222-222222222222";
const name = (id, tool) => `channel_${id.replaceAll("-", "")}.${tool}`;
function descriptor(providerId, calls) {
  return {
    providerId,
    description: "Provider instructions",
    inputSchema: { type: "object", properties: { text: { type: "string" } }, required: ["text"] },
    execute: async (input, execution) => {
      calls.push({ input, execution });
      return { accepted: true };
    },
  };
}

test("current is exactly the inbound connection, while other providers remain callable", async () => {
  const calls = [];
  const catalog = {
    [name(one, "sendMessage")]: descriptor("slack", calls),
    [name(two, "replyToMessage")]: descriptor("agentmail", calls),
  };
  const channels = createChannels(
    catalog,
    { connectionId: one, providerId: "slack", externalId: "C123" },
    { delivery: { externalMessageId: "171.1" } },
  );
  assert.deepEqual(Object.keys(channels.current), ["sendMessage"]);
  assert.equal(channels.current.sendMessage.providerId, "slack");
  assert.match(channels.current.sendMessage.description, /Inbound message reference: 171.1/);
  await channels.slack.sendMessage(
    { channelId: "C123", text: "Slack reply" },
    { toolCallId: "programmatic-call" },
  );
  await channels.agentmail.replyToMessage({ messageId: "mail", text: "Email reply" });
  assert.equal(calls[0].execution.toolCallId, "programmatic-call");
  assert.equal(calls[1].input.text, "Email reply");
  assert.equal(channels.github, undefined);
  assert.equal(channels.connections().length, 2);
  const previous = channels.current.sendMessage;
  delete catalog[name(one, "sendMessage")];
  assert.deepEqual(Object.keys(channels.current), [], "No fallback to another connection");
  await assert.rejects(async () => previous({ text: "removed" }), /no longer available/);
});

test("duplicate providers require explicit selection unless one is the inbound connection", () => {
  const catalog = {
    [name(one, "sendMessage")]: descriptor("slack", []),
    [name(two, "sendMessage")]: descriptor("slack", []),
  };
  const channels = createChannels(catalog);
  assert.throws(() => channels.slack, /Multiple slack connections/);
  assert.equal(channels.forConnection(two).sendMessage.toolName, name(two, "sendMessage"));
  assert.equal(
    createChannels(catalog, { connectionId: one, providerId: "slack" }).slack.sendMessage.toolName,
    name(one, "sendMessage"),
  );
});

test("custom provider keys and tool names route through the published catalog", async () => {
  const calls = [];
  const toolName = name(one, "customer_action");
  const channels = createChannels({ [toolName]: descriptor("customer/chat", calls) });
  await channels.call_channel_tool(toolName, JSON.stringify({ custom: [1, 2] }), {
    toolCallId: "custom-id",
  });
  await channels.provider("customer/chat").customer_action({ custom: "direct" });
  assert.deepEqual(calls[0], { input: { custom: [1, 2] }, execution: { toolCallId: "custom-id" } });
  await assert.rejects(channels.call_channel_tool(toolName, "private invalid input"), /valid JSON/);
  await assert.rejects(channels.call_channel_tool("__proto__", "{}"), /not available/);
});

test("native current exposes provider tools only, and instructions can be changed", async () => {
  const calls = [];
  const channels = createChannels({
    sendMessage: descriptor("native", calls),
    stop: { description: "Lifecycle", execute: async () => {} },
  });
  assert.deepEqual(Object.keys(channels.current), ["sendMessage"]);
  channels.current.sendMessage.description = "Customized instructions";
  assert.equal(channels.current.sendMessage.description, "Customized instructions");
  await channels.native.sendMessage({ text: "Native" });
  assert.equal(calls[0].input.text, "Native");
});
