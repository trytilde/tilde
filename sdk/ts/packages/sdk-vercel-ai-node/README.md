# Vercel AI SDK adapter

Core history and delivery live in `@trytilde/sdk`. This package converts typed
context to Vercel UI messages, following Dispatch's core/framework separation.

```ts
import { convertToAiSdkMessages, convertToAiSdkTools } from "@trytilde/sdk-vercel-ai-node";
import { convertToModelMessages, generateText, stepCountIs } from "ai";

const history = await ctx.message.history();
const messages = await convertToAiSdkMessages({ messages: history.items, context: ctx });
await generateText({
  model,
  messages: await convertToModelMessages(messages),
  tools: convertToAiSdkTools(ctx.channel.current),
  system: "Respond using the current channel's tools. Returned model text is private.",
  stopWhen: stepCountIs(8),
});
```

History pages are chronological; pass `beforeMessageId: history.nextPageToken`
for older messages. The latest page includes the current objective unless it
already matches the latest received message. `includeObjective: false` omits it.
`includeWork: true` also reads current goals/tasks and requires `work.read`.
Only the acting agent's messages receive the assistant role.

Images and PDFs are downloaded through `ctx.attachments.download` and embedded as
file parts. Text files include their real content. Unsupported binary formats get
an explicit attachment description; use `onAttachment` to parse them yourself.
No private URL or credential needs to be exposed to the model.

```ts
const history = await ctx.message.history({ includeWork: true });
const messages = await convertToAiSdkMessages({
  messages: history.items,
  context: ctx,
  onMessage: {
    goal: ({ id, goal }) => ({
      id, role: "user", parts: [{ type: "text", text: `Our goal: ${goal.objective}` }],
    }),
    task: () => null, // Omit this type, or provide a different rendering.
  },
  onAttachment: async ({ attachment, download }) => {
    const { content } = await download();
    return { type: "text", text: decodeYourFormat(attachment, content) };
  },
});
```

`onMessage` supports `message`, `objective`, `goal`, and `task`. Supplied handlers
take precedence over cached/default rendering; returning null omits an item.
Without an override the converter renders all supported types. Completed
conversation conversions use the existing per-agent cache, in bounded batches.
Files are hydrated afresh and are never stored as large data URLs in the cache;
objectives/goals/tasks remain live projections rather than cached chat records.


## Channel tools

`convertToAiSdkTools(ctx.channel.current)` preserves the provider's descriptions
and JSON schemas and forwards the AI SDK tool-call ID to Tilde's audited tool
execution. It does not publish the model's final text. The agent chooses the
provider tool and arguments, including routing fields required by that provider.

Override tool instructions with `instructions: { sendMessage: "..." }`, or change
the channel tool's `description` before conversion. When combining namespaces,
use `prefix: "slack_"` (or another prefix) to keep model tool names distinct.

The core SDK exposes typed callable tools on `ctx.channel.slack`, `github`,
`agentmail`, `linq`, `whatsapp`, `telnyxWhatsapp`, and `native`. These collections
contain only server-published tools; optional methods reflect the agent's grants.
Use `ctx.channel.connections()` and `ctx.channel.forConnection(id)` when multiple
connections use a provider. `ctx.channel.current` never falls back to a different
connection when the inbound connection has no available tools.

```ts
if (ctx.channel.slack?.sendMessage) {
  await ctx.channel.slack.sendMessage({ channelId: "C123", text: "Hello" });
}
await ctx.channel.call_channel_tool(publishedToolName, JSON.stringify(customArguments));
```

Custom provider names are available through `ctx.channel.provider(providerId)`.
The dynamic call path accepts serialized JSON and the exact published tool name;
the API still validates the schema and invocation authorization. Programmatic
calls may supply `{ toolCallId }`; the runtime rejects repeated IDs to prevent
duplicate execution. Provider result bodies
remain their original JSON instead of being coerced into a generic message type.
