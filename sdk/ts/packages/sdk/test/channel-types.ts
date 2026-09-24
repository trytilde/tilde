import type { AgentContext } from "../dist/index.js";
declare const ctx: AgentContext;
ctx.channel.slack?.sendMessage?.({ channelId: "C123", text: "Hello" });
ctx.channel.agentmail?.replyToMessage?.({ messageId: "mail", text: "Hello" });
ctx.channel.telnyxWhatsapp?.sendMessage?.({ to: "+15550001111", text: "Hello" });
ctx.channel.github?.sendMessage?.({ owner: "owner", repository: "repo", number: 1, text: "Hello" });
ctx.channel.call_channel_tool("published_custom_tool", JSON.stringify({ customer: "value" }));
// @ts-expect-error Slack requires its provider-specific channelId.
ctx.channel.slack?.sendMessage?.({ text: "Hello" });
// @ts-expect-error AgentMail recipients are email arrays.
ctx.channel.agentmail?.sendMessage?.({ to: "one@example.com", subject: "Hi", text: "Hello" });
ctx.channel.github?.sendMessage?.({
  owner: "owner",
  repository: "repo",
  // @ts-expect-error GitHub issue numbers are numeric.
  number: "1",
  text: "Hello",
});
// @ts-expect-error Only Meta WhatsApp has a markRead tool.
ctx.channel.telnyxWhatsapp?.markRead?.({ messageId: "message" });

ctx.channel.provider("slack", "connection-id")?.sendMessage?.({ channelId: "C123", text: "Hello" });
// @ts-expect-error Explicitly selecting a built-in connection retains its argument types.
ctx.channel.provider("slack", "connection-id")?.sendMessage?.({ text: "Missing channel" });
ctx.channel.provider("customer/chat")?.customAction({ customer: "dynamic" });

// @ts-expect-error The descriptor's execute method is typed too.
ctx.channel.slack?.sendMessage?.execute({ text: "Missing channel" });
