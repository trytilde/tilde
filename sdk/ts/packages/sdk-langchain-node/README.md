# LangChain / LangGraph adapter

Core history and delivery live in `@trytilde/sdk`. This package converts typed
context to LangChain messages and tools, following Dispatch's core/framework
separation. It works with `langchain` v1 (`createAgent`) and `@langchain/core` 1.x.

Define the agent once at module scope, with its model built from module-level
`inference(alias)`, and add `tildeMiddleware()`: `createAgent` fixes its tools and
middleware when it builds the graph, so the middleware brings each invocation's pieces.

```ts
import { inference } from "@trytilde/sdk";
import {
  convertToLangChainMessages,
  tildeLangChain,
  tildeMiddleware,
} from "@trytilde/sdk-langchain-node";
import { ChatOpenAI } from "@langchain/openai";
import { createAgent } from "langchain";

const { baseURL, apiKey, fetch } = inference("default");
export const agent = createAgent({
  name: "my-agent",
  model: new ChatOpenAI({ apiKey, configuration: { baseURL, fetch }, model: "gpt-4o-mini" }),
  systemPrompt: "Respond using the current channel's tools. Returned model text is private.",
  middleware: [tildeMiddleware()],
});

export async function run(ctx: AgentContext) {
  const history = await ctx.message.history();
  const messages = await convertToLangChainMessages({ messages: history.items, context: ctx });
  await agent.invoke({ messages }, { ...tildeLangChain(ctx), recursionLimit: 16 });
}
```

`tildeLangChain(ctx)` is the call's `signal` and a `configurable` entry carrying the
invocation (spread it when you pass your own `configurable`, e.g. a `thread_id`).
`tildeMiddleware()` then, per model call, adds the current channel's tools and runs them
(tools added in `wrapModelCall` execute through its `wrapToolCall`), adds steering input
sent while the agent works to the agent state as user messages before each model call,
and offers the invocation's skills: LangChain has no native skills, so it adds Tilde's
`list_skills`/`read_skill` tools and appends `ctx.skills.summary()` to the system message.
Without `tildeLangChain(ctx)` in the call the middleware refuses to run.

`tilde deploy` reads exported `createAgent` agents (`systemPrompt` as the plain prompt
`<name or export>/system_prompt`), `PromptTemplate`s (`<export>`: f-string as braces,
mustache as mustache) and `ChatPromptTemplate`s (`<export>/<message index>`). A
`dynamicSystemPromptMiddleware` function sits in a closure it cannot read, so it is
reported as a warning; declare such prompts with `definePrompt` to version them.

The messages and tools also work with `createReactAgent({ llm, tools })` from
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

## The agent's tools

`withTildeTools(ctx, tools, options?)` returns one `StructuredToolInterface[]` for
`createAgent`, `createReactAgent` or `bindTools`: the current channel's tools, `ctx.agentTools`
and `tools`, the agent's own LangChain tools. Those are published to Tilde for the invocation,
so `tools.search` and `tools.schemas` describe them and a `tools.execute` naming one runs it here
with the graph's config; each call is audited once with the model's call ID. Summary, display
and annotations come from the tool's `metadata.tilde`, overridden by `options[name]`. LangChain
tools have no output schema.

```ts
const tools = await withTildeTools(ctx, [
  tool(async ({ sides }) => ({ roll: 1 + Math.floor(Math.random() * sides) }), {
    name: "roll_dice",
    description: "Roll a die.",
    schema: z.object({ sides: z.number().int() }),
    metadata: { tilde: { summary: "Rolled a die", display: "summary" } },
  }),
]);
```

Input rejected by the tool's schema, and a returned `ToolMessage` with status `error`, are
audited as failed. Tools that read LangGraph-injected state can only run inside the graph, so a
`tools.execute` naming one runs without that state.

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
