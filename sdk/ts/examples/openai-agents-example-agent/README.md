Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent OpenAI Agents** (`0b6f3c52-3f1d-4b9a-8c47-5e2d9a7c4b03`) and starts it
beside the other SDK examples. Set `DEV_AGENTS=openai-agents` to start only this one.

A private SDK example that answers with the OpenAI Agents SDK (`@openai/agents`)
instead of the Vercel AI SDK. Override `OPENAI_MODEL` as needed. It needs `OPENAI_API_KEY`.
It dials out to Tilde with `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (a Gateway deployment token for the agent) and listens on no port. Keys and tokens are never logged.

The model receives only `ctx.channel.current` tools and responds by calling them.
Returned model text remains private; visible responses are provider tool actions.
Native threads and all six built-in channel providers are supported. Attachments are
downloaded through the scoped SDK and converted into `input_image`, `input_file` or
`input_text` content by `@trytilde/sdk-openai-agents-node`. Assign channels in Tilde
normally.

This private TypeScript package is part of the SDK workspace. `src/index.ts` configures
the Tilde connection, the default OpenAI key, and disables the OpenAI Agents trace exporter
(it would upload prompts and tool data to OpenAI; Tilde collects OTel spans itself).
`src/agent.ts` builds an `Agent` per invocation and runs it with
`run(agent, items, { signal: ctx.signal, maxTurns: 8 })`. History, typed context
conversion, attachments and reply routing are owned by the SDK through
`ctx.message.history()`, `convertToOpenAIAgentsMessages` and
`convertToOpenAIAgentsTools(ctx.channel.current)`.

Build and start it on its own:

```sh
pnpm --dir sdk/ts --filter @trytilde/openai-agents-example-agent... build
OPENAI_API_KEY=... TILDE_GATEWAY_URL=... TILDE_DEPLOYMENT_TOKEN=... \
  node sdk/ts/examples/openai-agents-example-agent/dist/index.js
```

Create the agent in Tilde and register a Gateway deployment for the token, granting
`toolsInvoke`, `workRead`, `workWrite` and `runUpdate` as for `sdk/ts/examples/vercel-ai-example-agent`.
