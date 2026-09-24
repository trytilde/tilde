Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent LangChain** (`5f0c1f0e-7a57-4d0b-9f6e-3d1c8b1f2a02`) and starts it
beside the other SDK examples. Set `DEV_AGENTS=langchain` to start only this one.

A LangChain/LangGraph counterpart to `sdk/ts/examples/vercel-ai-example-agent`. Start it with
`OPENAI_API_KEY` set; it uses `OPENAI_MODEL` (default `gpt-4o-mini`).
It dials out to Tilde with `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (a Gateway deployment token for the agent) and listens on no port. Keys and tokens are never logged.

The model receives only `ctx.channel.current` tools and responds by calling them.
Returned model text remains private; visible responses are provider tool actions.
Native threads and all six built-in channel providers are supported. Attachments
are downloaded through the scoped SDK and converted into LangChain image, file or
text content blocks by `@trytilde/sdk-langchain-node`. Assign channels in Tilde normally.

This private TypeScript package is part of the SDK workspace. `src/index.ts`
configures the Tilde connection and `ChatOpenAI`; `src/agent.ts` runs a `createAgent`
(LangChain v1, LangGraph underneath) loop with `ctx.signal` as the graph signal and a
capped `recursionLimit`. History, typed context conversion, attachments and reply
routing are owned by the SDK through `ctx.message.history()`,
`convertToLangChainMessages` and `convertToLangChainTools(ctx.channel.current)`.
Logging mirrors vercel-ai-example-agent: Pino bridged into OTel through `agentLogProcessor`.

Build on its own with `pnpm --dir sdk/ts --filter @trytilde/langchain-example-agent... build`.
