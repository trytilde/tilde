# Vercel AI SDK adapter

Core history and delivery live in `@trytilde/sdk`. Write the AI SDK `ToolLoopAgent` the way
the AI SDK docs show, at module scope, and pass the per-invocation Tilde pieces as call options
with `tildeAiSdk(ctx)`.

```ts
import { connectAgent, inference } from "@trytilde/sdk";
import { convertToAiSdkMessages, tildeAiSdk, tildeCallOptions } from "@trytilde/sdk-vercel-ai-node";
import { ToolLoopAgent, convertToModelMessages, stepCountIs } from "ai";
import { createOpenAI } from "@ai-sdk/openai";

export const agent = new ToolLoopAgent({
  id: "assistant",
  instructions: "Respond using the current channel's tools. Returned model text is private.",
  model: createOpenAI(inference("openai/prod")).responses("gpt-4o-mini"),
  stopWhen: stepCountIs(8),
  ...tildeCallOptions,
});

connectAgent({
  run: async (ctx) => {
    const history = await ctx.message.history();
    const messages = await convertToAiSdkMessages({ messages: history.items, context: ctx });
    await agent.generate({ messages: await convertToModelMessages(messages), ...tildeAiSdk(ctx) });
  },
});
```

`agent.generate/stream` take no per-call tools or instructions (ai 6.x); the AI SDK's way to
vary them per call is call options (`callOptionsSchema` + `prepareCall`), which
`tildeCallOptions` supplies. `tildeAiSdk(ctx, toolOptions?)` returns `options: { tilde }` and
`abortSignal: ctx.signal`; the agent's `prepareCall` then adds, on top of its own settings:

- the current channel's tools (`convertToAiSdkTools`, `toolOptions` as there);
- the agent's Tilde skills: the AI SDK has no native skills, so they come as `list_skills` and
  `read_skill` tools plus `ctx.skills.summary()` appended to `instructions` (as its own system
  message when the instructions are messages). Skills assigned in the registry reach running
  deployments on their next invocation;
- a `prepareStep` (wrapping the agent's own) that hands steering input sent while the agent
  works to the model as user messages before every step; the AI SDK rebuilds each step's
  messages, so handed-over inputs are re-inserted where they arrived.

An agent with call options of its own adds `tilde` to its schema and returns
`prepareTildeCall(options.tilde.context, settings, options.tilde.tools)` from its
`prepareCall`. With `generateText`/`streamText`, call it directly:

```ts
await generateText({
  ...(await prepareTildeCall(ctx, { model, system, messages })),
  stopWhen: stepCountIs(8),
  abortSignal: ctx.signal,
});
```

## Deploy-time discovery

`tilde deploy` calls this package's `discover` for each export of the entry module. A
`ToolLoopAgent` contributes `<agent id, or the export name>/instructions`: a string or system
message as a plain prompt (several system messages as `…/instructions/<i>`, since each is sent
alone). ToolLoopAgent instructions cannot be functions, so there is nothing to stamp at run
time; the gateway matches the static text. The AI SDK has no getter for them: discovery reads
the TypeScript-private `settings` (verified against ai 6.0.280) and warns when it is missing, or
when the agent has its own `prepareCall` that may replace them.

`discoverProject` reads a Vercel eve project (`agent/` beside the entry's `package.json`, or in
the working directory): `agent/instructions.md` as the plain prompt `agent/instructions`; else
`agent/instructions.{ts,mjs,js}`, whose default (or `instructions`) export is plain text, or a
function registered as a dynamic prompt with its source; and `agent/skills/<name>/SKILL.md` as
skills. eve runs its own runtime, so only discovery applies to it.

## History

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


## The agent's tools

`withTildeTools(ctx, tools, options?)` returns one `ToolSet` for `generateText`/`streamText`:
the current channel's tools, `ctx.agentTools` and `tools`, the agent's own AI SDK tools keyed by
the name the model sees. Those are published to Tilde for the invocation, so `tools.search` and
`tools.schemas` describe them and a `tools.execute` naming one runs it here (validated with the
tool's input schema first); each call is audited once with the model's call ID. Summary, display
and annotations come from the tool's `metadata.tilde` (not sent to the model), overridden by
`options[name]`. JSON schemas come from `asSchema(...).jsonSchema`.

```ts
const tools = await withTildeTools(ctx, {
  roll_dice: tool({
    description: "Roll a die.",
    inputSchema: z.object({ sides: z.number().int() }),
    metadata: { tilde: { summary: "Rolled a die", display: "summary" } },
    execute: async ({ sides }) => ({ roll: 1 + Math.floor(Math.random() * sides) }),
  }),
});
```

Provider-defined tools and tools without `execute` pass through unpublished and unaudited. An
`execute` returning an AsyncIterable is drained: only its final value reaches the model and the
audit.

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
