Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent Mastra** (`c1d2a9e4-6b38-4f75-a1c0-8f4e6d2b9c04`) and starts it
beside the other SDK examples. Set `DEV_AGENTS=mastra` to start only this one.

A Mastra counterpart of `sdk/ts/examples/vercel-ai-example-agent`. Start it as a plain SDK host with
`OPENAI_API_KEY` and optionally `OPENAI_MODEL` (default `gpt-4o-mini`).
It dials out to Tilde with `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (a Gateway deployment token for the agent) and listens on no port. Keys and tokens are never logged.

The model receives only `ctx.channel.current` tools and responds by calling them.
Returned model text remains private; visible responses are provider tool actions.
Native threads and all six built-in channel providers are supported. Attachments
are downloaded through the scoped SDK and converted into model file or text parts.

This private TypeScript package is part of the SDK workspace. `src/index.ts`
configures the Tilde connection and the Vercel AI SDK OpenAI provider; `src/agent.ts`
builds a Mastra `Agent` per invocation and generates replies with `agent.generate`,
bounded to 8 steps and cancelled through `ctx.signal`. History, typed context
conversion, attachments and reply routing are owned by the SDK through
`ctx.message.history()`, `convertToMastraMessages` and
`convertToMastraTools(ctx.channel.current)` from `@trytilde/sdk-mastra-node`.
Logging mirrors vercel-ai-example-agent: Pino bridged to OTel through `agentLogProcessor`.

Build on its own with `pnpm --dir sdk/ts --filter @trytilde/mastra-example-agent... build`.
