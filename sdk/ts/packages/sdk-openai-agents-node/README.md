# OpenAI Agents SDK adapter

Core history and delivery live in `@trytilde/sdk`. This package converts typed
context to OpenAI Agents input items and function tools, following Dispatch's
core/framework separation.

Define the agent once at module scope, with its model built from module-level
`inference(alias)`, and run a per-invocation copy from `tildeOpenAIAgents`:

```ts
import { inference } from "@trytilde/sdk";
import {
  convertToOpenAIAgentsMessages,
  tildeOpenAIAgents,
} from "@trytilde/sdk-openai-agents-node";
import { Agent, OpenAIResponsesModel, run } from "@openai/agents";
import OpenAI from "openai";

export const agent = new Agent({
  name: "my-agent",
  model: new OpenAIResponsesModel(new OpenAI(inference("default")), "gpt-4o-mini"),
  instructions: "Respond using the current channel's tools. Returned model text is private.",
});

export async function respond(ctx: AgentContext) {
  const history = await ctx.message.history();
  const items = await convertToOpenAIAgentsMessages({ messages: history.items, context: ctx });
  const tilde = await tildeOpenAIAgents(ctx, agent);
  await run(tilde.agent, items, { ...tilde.options, maxTurns: 8 });
}
```

`tilde.agent` is a clone of the agent and of every agent it hands off to (cycles
included), each with the current channel's tools and the invocation's skills: an agent
with a local `shellTool` gets registry skills (`ctx.skills.directory()`) as local shell
skills; others get Tilde's `list_skills`/`read_skill` tools with `ctx.skills.summary()`
appended to their instructions. `tilde.options` is the invocation's `signal` and a
`callModelInputFilter` that adds steering input as user messages before each model call
(kept in place for later turns) and stamps the running agent's dynamic instructions.
Agents used as tools (`asTool()`) run nested runs with their own options and get neither.

`tilde deploy` reads exported agents and those reachable through handoffs and agent tools:
`<name>/instructions` (a string as plain text, a function as dynamic with its source),
`<name>/handoff_description`, shell tool local skills and `local_dir` sandbox skills. A
hosted `prompt: { promptId }` is versioned by OpenAI and reported as a warning.

History pages are chronological; pass `beforeMessageId: history.nextPageToken`
for older messages. The latest page includes the current objective unless it
already matches the latest received message. `includeObjective: false` omits it.
`includeWork: true` also reads current goals/tasks and requires `work.read`.
Only the acting agent's messages receive the assistant role; they become completed
assistant items with `output_text` content.

Images are downloaded through `ctx.attachments.download` and embedded as
`input_image` data URLs, PDFs as `input_file` data with their filename. Text files
include their real content. Unsupported binary formats get an explicit attachment
description; use `onAttachment` to parse them yourself. Assistant items cannot carry
input files, so the agent's own attachments are named rather than embedded. No
private URL or credential needs to be exposed to the model.

```ts
const history = await ctx.message.history({ includeWork: true });
const items = await convertToOpenAIAgentsMessages({
  messages: history.items,
  context: ctx,
  onMessage: {
    goal: ({ goal }) => ({
      role: "user",
      content: [{ type: "input_text", text: `Our goal: ${goal.objective}` }],
    }),
    task: () => null, // Omit this type, or provide a different rendering.
  },
  onAttachment: async ({ attachment, download }) => {
    const { content } = await download();
    return { type: "input_text", text: decodeYourFormat(attachment, content) };
  },
});
```

`onMessage` supports `message`, `objective`, `goal`, and `task`. Supplied handlers
take precedence over cached/default rendering; returning null omits an item.
Without an override the converter renders all supported types. Completed
conversation conversions use the existing per-agent cache, in bounded batches.
Files are hydrated afresh and are never stored as large data URLs in the cache;
objectives/goals/tasks remain live projections rather than cached chat records.

Returned items carry no `id`. The OpenAI Responses model forwards item ids verbatim
and the API rejects ids it did not issue, so the Tilde message id is stored only on
the cached representation, where hydration checks it against the canonical message.

## The agent's tools

`withTildeTools(ctx, tools, options?)` returns one `Tool[]` for an `Agent`: the current
channel's tools, `ctx.agentTools` and `tools`, the agent's own `tool()` function tools. Those are
published to Tilde for the invocation, so `tools.search` and `tools.schemas` describe them and a
`tools.execute` naming one runs it here with the run's `RunContext`; each call is audited once
with the model's call ID. Function tools carry no metadata, so summary, display and annotations
are given as `options[name]`.

```ts
const tools = await withTildeTools(
  ctx,
  [
    tool({
      name: "roll_dice",
      description: "Roll a die.",
      parameters: z.object({ sides: z.number().int() }),
      execute: async ({ sides }) => ({ roll: 1 + Math.floor(Math.random() * sides) }),
    }),
  ],
  { roll_dice: { summary: "Rolled a die", display: "summary" } },
);
```

A tool's `errorFunction` (the default for tools without an output schema) turns a thrown error
into a string result, so that failure is audited as completed with the error text. Hosted tools
pass through unpublished and unaudited.

## Channel tools

`convertToOpenAIAgentsTools(ctx.channel.current)` returns `tool()` definitions in
non-strict mode, so the provider's descriptions and JSON schemas reach the model
unchanged, and forwards the model's function-call ID (`details.toolCall.callId`)
to Tilde's audited tool execution. Execution honors the run's abort signal. It does
not publish the model's final text. The agent chooses the provider tool and
arguments, including routing fields required by that provider.

Override tool instructions with `instructions: { sendMessage: "..." }`, or change
the channel tool's `description` before conversion. When combining namespaces,
use `prefix: "slack_"` (or another prefix) to keep model tool names distinct.
OpenAI Agents rewrites every non-alphanumeric character in a tool name to `_`;
names that collide after rewriting are rejected.

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
duplicate execution. Provider result bodies remain their original JSON instead of
being coerced into a generic message type.

## Tracing

`@openai/agents` exports traces to OpenAI by default, including prompts and tool
data. Call `setTracingDisabled(true)` (or set `OPENAI_AGENTS_DISABLE_TRACING=1`)
when Tilde's OTel collection is the system of record, as `sdk/ts/examples/openai-agents-example-agent` does.
