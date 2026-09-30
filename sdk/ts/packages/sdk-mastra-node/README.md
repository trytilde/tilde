# Mastra adapter

Core history and delivery live in `@trytilde/sdk`. Write the Mastra `Agent` the way Mastra's
docs show, at module scope, and pass the per-invocation Tilde pieces as call options with
`await tildeMastra(ctx)`. Mastra runs on the Vercel AI SDK, so message conversion is delegated to
`@trytilde/sdk-vercel-ai-node`; the returned UI messages are accepted by `agent.generate()` /
`agent.stream()` directly.

```ts
import { connectAgent, inference } from "@trytilde/sdk";
import { convertToMastraMessages, tildeMastra, tildeMastraSkills } from "@trytilde/sdk-mastra-node";
import { Agent } from "@mastra/core/agent";
import { createOpenAI } from "@ai-sdk/openai";

export const agent = new Agent({
  id: "assistant",
  name: "Assistant",
  instructions: "Respond using the current channel's tools. Returned model text is private.",
  model: createOpenAI(inference("openai/prod")).responses("gpt-4o-mini"),
  skills: tildeMastraSkills([fileURLToPath(new URL("../skills", import.meta.url))]),
});

connectAgent({
  run: async (ctx) => {
    const history = await ctx.message.history();
    const messages = await convertToMastraMessages({ messages: history.items, context: ctx });
    await agent.generate(messages, { ...(await tildeMastra(ctx)), maxSteps: 8 });
  },
});
```

`tildeMastra(ctx, options?)` returns `toolsets: { tilde: <channel, agent and bundled tools> }`
(the agent's own tools from `options.bundled`, a `defineTools(...)`; see `withTildeTools`),
`abortSignal: ctx.signal` and one input processor that, before every step, adds steering
input sent during the run as user messages and stamps dynamic instructions (below) on the
step's model call. Per-call `inputProcessors` replace the agent's own configured ones
(Mastra's memory and skills processors stay), so list yours alongside it if the agent has any.

## Registry skills

Skills assigned to the agent in Tilde (sources, single skills, connection skills) reach every
running deployment on its next invocation, with no redeploy. Mastra has no per-call skills
option, so the agent opts in with Mastra's dynamic `skills` form, one line:
`skills: tildeMastraSkills([...your skills])`. `tildeMastra(ctx)` puts the invocation in the
call's `requestContext` (pass your own as `tildeMastra(ctx, { requestContext })`); the
resolver then adds `ctx.skills.directory()`, the registry skills written to a cache directory
that is reused while their versions are unchanged. Skills the deployment shipped are not
fetched again. Outside an invocation, `agent.listSkills()` and so `tilde deploy` see only the
listed skills, which ship as the deployment's bundled skills.

## Deploy-time discovery

`tilde deploy` calls this package's `discover` for each export of the entry module. An
`Agent` (or each agent of an exported `Mastra`) contributes:

- `<agent id>/instructions`: static instructions as a plain prompt (a string, or messages
  joined as Mastra joins them); an instructions function as a dynamic prompt whose template is
  the function's source. The function is never called.
- its skills (`agent.listSkills()`, including workspace skills): path skills as their
  directories, resolved like Mastra against the working directory (prefer absolute paths);
  `createSkill()` skills as a generated `SKILL.md` plus `references/`.

Mastra has no public getter for unevaluated instructions; discovery reads
`__getOverridableFields()` (verified against @mastra/core 1.67) and warns when it is missing.

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

## The agent's tools

`withTildeTools(ctx, tools, options?)` returns one tools record for a Mastra `Agent`: the
current channel's tools, `ctx.agentTools` and `tools`, the agent's own `createTool` tools. The
record key is the name the model sees and the name Tilde publishes. Those are published to Tilde
for the invocation, so `tools.search` and `tools.schemas` describe them and a `tools.execute`
naming one runs it here with the outer call's `requestContext`; each call is audited once with
the model's call ID. `mcp.annotations` become Tilde's annotations (with MCP's defaults), summary
and display come from `mcp._meta.tilde`, and `options[name]` overrides both.

```ts
const tools = await withTildeTools(ctx, {
  roll_dice: createTool({
    id: "roll_dice",
    description: "Roll a die.",
    inputSchema: z.object({ sides: z.number().int() }),
    mcp: { annotations: { readOnlyHint: true }, _meta: { tilde: { summary: "Rolled a die" } } },
    execute: async ({ sides }) => ({ roll: 1 + Math.floor(Math.random() * sides) }),
  }),
});
const agent = new Agent({ id: "assistant", name: "Assistant", instructions, model, tools });
```

Mastra returns a `ValidationError` for invalid input or output instead of throwing; it is
audited as failed. A routed `tools.execute` runs outside the agent loop, without
suspend/resume. Entries that are not Mastra tools pass through unpublished and unaudited.

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
