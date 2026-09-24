# LangChain / LangGraph adapter

Core history and delivery live in `@trytilde/sdk`. This package converts typed
context to LangChain messages and tools, following Dispatch's core/framework
separation. It depends only on `@langchain/core` 1.x; bring your own chat model
and agent loop (`createAgent` from `langchain` v1 or `createReactAgent` from
`@langchain/langgraph/prebuilt`).

```ts
import { convertToLangChainMessages, convertToLangChainTools } from "@trytilde/sdk-langchain-node";
import { createAgent } from "langchain";

const history = await ctx.message.history();
const messages = await convertToLangChainMessages({ messages: history.items, context: ctx });
const agent = createAgent({
  model,
  tools: convertToLangChainTools(ctx.channel.current),
  systemPrompt: "Respond using the current channel's tools. Returned model text is private.",
});
await agent.invoke({ messages }, { signal: ctx.signal, recursionLimit: 16 });
```

The same messages and tools work with `createReactAgent({ llm, tools })` from
`@langchain/langgraph/prebuilt`, or directly with `model.bindTools(tools)`.

History pages are chronological; pass `beforeMessageId: history.nextPageToken`
for older messages. The latest page includes the current objective unless it
already matches the latest received message. `includeObjective: false` omits it.
`includeWork: true` also reads current goals/tasks and requires `work.read`.
Only the acting agent's messages become `AIMessage`; everything else is a
`HumanMessage`. Every converted message carries the SDK item id as its `id`.

Images and PDFs are downloaded through `ctx.attachments.download` and embedded as
standard `image`/`file` content blocks with base64 data and a MIME type. Text files
include their real content. Unsupported binary formats get an explicit attachment
description; use `onAttachment` to parse them yourself. No private URL or
credential needs to be exposed to the model.

```ts
import { HumanMessage } from "@langchain/core/messages";

const history = await ctx.message.history({ includeWork: true });
const messages = await convertToLangChainMessages({
  messages: history.items,
  context: ctx,
  onMessage: {
    goal: ({ id, goal }) => new HumanMessage({ id, content: `Our goal: ${goal.objective}` }),
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
conversation conversions use the existing per-agent cache, in bounded batches,
stored as plain JSON (`id`, `role`, text content) rather than serialized LangChain
classes. Files are hydrated afresh and are never stored as base64 in the cache;
objectives/goals/tasks remain live projections rather than cached chat records.

## Channel tools

`convertToLangChainTools(ctx.channel.current)` returns `StructuredToolInterface[]`
built with `tool()` from `@langchain/core/tools`. It preserves the provider's
descriptions and JSON schemas and forwards the model's tool-call ID to Tilde's
audited tool execution: LangGraph's `ToolNode` and `createAgent` invoke each tool
with the `ToolCall`, which `@langchain/core` exposes as `config.toolCall.id`. A
direct `invoke(args)` without a tool call gets a fresh ID. Tool execution honors
`config.signal` and refuses to start once it is aborted; pass `ctx.signal` as the
graph's `signal`. It does not publish the model's final text. The agent chooses the
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
duplicate execution. Provider result bodies remain their original JSON, serialized
into the `ToolMessage` content, instead of being coerced into a generic message type.
