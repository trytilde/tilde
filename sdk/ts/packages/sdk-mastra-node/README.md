# Mastra adapter

Core history and delivery live in `@trytilde/sdk`. This package exposes typed
context and channel tools to a Mastra `Agent`. Mastra runs on the Vercel AI SDK,
so message conversion is delegated to `@trytilde/sdk-vercel-ai-node`; the
returned UI messages are accepted by `agent.generate()` / `agent.stream()`
directly, without a `convertToModelMessages` step.

```ts
import { convertToMastraMessages, convertToMastraTools } from "@trytilde/sdk-mastra-node";
import { Agent } from "@mastra/core/agent";

const history = await ctx.message.history();
const messages = await convertToMastraMessages({ messages: history.items, context: ctx });
const agent = new Agent({
  id: "assistant",
  name: "Assistant",
  instructions: "Respond using the current channel's tools. Returned model text is private.",
  model,
  tools: convertToMastraTools(ctx.channel.current),
});
await agent.generate(messages, { maxSteps: 8, abortSignal: ctx.signal });
```

History pages are chronological; pass `beforeMessageId: history.nextPageToken`
for older messages. The latest page includes the current objective unless it
already matches the latest received message. `includeObjective: false` omits it.
`includeWork: true` also reads current goals/tasks and requires `work.read`.
Only the acting agent's messages receive the assistant role.

Images and PDFs are downloaded through `ctx.attachments.download` and embedded as
file parts. Text files include their real content. Unsupported binary formats get
an explicit attachment description; use `onAttachment` to parse them yourself.
`onMessage` supports `message`, `objective`, `goal`, and `task`; returning null
omits an item. `convertAttachment`, `AttachmentHandler`, `AttachmentConversion`
and `MessageHandlers` are re-exported from the Vercel adapter, whose README
documents the conversion and caching behaviour in full.

```ts
const history = await ctx.message.history({ includeWork: true });
const messages = await convertToMastraMessages({
  messages: history.items,
  context: ctx,
  onMessage: {
    goal: ({ id, goal }) => ({
      id, role: "user", parts: [{ type: "text", text: `Our goal: ${goal.objective}` }],
    }),
    task: () => null,
  },
  onAttachment: async ({ attachment, download }) => {
    const { content } = await download();
    return { type: "text", text: decodeYourFormat(attachment, content) };
  },
});
```

## Channel tools

`convertToMastraTools(ctx.channel.current)` returns a record of tools created
with `createTool` from `@mastra/core/tools`. Each tool keeps the provider's
description and passes the provider's JSON Schema straight through as
`inputSchema` (Mastra accepts plain JSON Schema and validates input with it
before `execute` runs). The tool ID is the sanitized model tool name. The agent
chooses the provider tool and arguments, including routing fields required by
that provider; it never publishes the model's final text.

Mastra executes tools through the AI SDK and exposes the model's tool-call ID
inside an agent run as `context.agent.toolCallId`. The adapter forwards that ID
to Tilde's audited `channel.execute(input, { toolCallId })`, so audit records
match the model's tool calls and the runtime rejects a repeated ID instead of
sending twice. Calling a converted tool's `execute` directly, outside an agent
run, has no model ID; the adapter generates a random UUID for that execution.
Mastra's `abortSignal` is honoured: an already-aborted signal rejects before the
channel is reached.

Override tool instructions with `instructions: { sendMessage: "..." }`, or change
the channel tool's `description` before conversion. When combining namespaces,
use `prefix: "slack_"` (or another prefix) to keep model tool names distinct.
Conflicting or invalid sanitized names throw.

The core SDK exposes typed callable tools on `ctx.channel.slack`, `github`,
`agentmail`, `linq`, `whatsapp`, `telnyxWhatsapp`, and `native`. Use
`ctx.channel.connections()` and `ctx.channel.forConnection(id)` when multiple
connections use a provider. `ctx.channel.current` never falls back to a different
connection when the inbound connection has no available tools.
