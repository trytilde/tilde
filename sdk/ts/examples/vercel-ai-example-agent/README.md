Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command
registers **Example Agent 1** (`9a97ef65-4f0c-4ce8-9d95-7b8aa39bca01`) and starts this
SDK host, which dials out to Tilde with `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN`
and listens on no port. Override `OPENAI_MODEL` as needed.
The same command starts one example per SDK adapter (TypeScript:
LangChain, OpenAI Agents, Mastra; Python: LangChain, Pydantic AI, OpenAI Agents, Agno), each
with its own stable ID; `DEV_AGENTS=vercel-ai` (or a comma-separated list of keys) limits which
ones start. Python examples run from the uv workspace in `sdk/py`, which the launcher syncs.
Deployment tokens and keys are never logged.

The launcher needs `OPENAI_API_KEY` in the ignored `.env.local`. The model receives only `ctx.channel.current`
tools and responds by calling them. Returned model text remains private; visible
responses are provider tool actions. Native threads and all six built-in channel
providers are supported. Attachments are downloaded through the scoped SDK and
converted into model file or text parts by `@trytilde/sdk-vercel-ai-node`. Assign channels in Tilde normally.

This private TypeScript package is part of the SDK workspace. `task dev` builds it
after `@trytilde/sdk` and runs `dist/index.js`. `src/agent.ts` defines the AI SDK
`ToolLoopAgent` once at module scope (instructions, model, 8-step limit, `...tildeCallOptions`)
and `respond(ctx)` calls `agent.generate({ messages, ...tildeAiSdk(ctx) })`, which adds the
channel tools, the agent's Tilde skills (tools plus a summary in the instructions), steering and
`ctx.signal`. `src/index.ts` exports the agent and connects; `tilde deploy --dry-run` prints its
instructions as the prompt `vercel-ai-example-agent/instructions`. History, typed context
conversion, attachments and reply routing are owned by the SDK through `ctx.message.history()`,
`convertToAiSdkMessages` and `@trytilde/sdk-vercel-ai-node`.

Build on its own with `pnpm --dir sdk/ts --filter @trytilde/vercel-ai-example-agent... build`.

Model calls go through Tilde's inference gateway: `inference(TILDE_INFERENCE)` (default `default`,
the alias the local registrar gives the shared OpenAI connection) builds the model client once;
each request goes through the invocation running when it is made, with its token, so the
process never holds a provider key.
