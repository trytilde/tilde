Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command
registers **Example Agent 1** (`9a97ef65-4f0c-4ce8-9d95-7b8aa39bca01`) and starts this
SDK server on `127.0.0.1:3001`. Override `DEV_AGENT_PORT` or `OPENAI_MODEL` as needed.
The existing registry key is decrypted only to start the child; keys are never logged.

`task secrets:load` reads `openai_api_key` into the private dev dotenv. Without that
file, the launcher reads SOPS directly. The model receives only `ctx.channel.current`
tools and responds by calling them. Returned model text remains private; visible
responses are provider tool actions. Native threads and all six built-in channel
providers are supported. Attachments are downloaded through the scoped SDK and
converted into model file or text parts by `@trytilde/sdk-vercel-ai-node`. Assign channels in Tilde normally.

This private TypeScript package is part of the SDK workspace. `task dev` builds it
after `@trytilde/sdk` and runs `dist/index.js`. `src/index.ts` configures the Tilde
server and Vercel AI SDK OpenAI provider; `src/agent.ts` generates replies with
`generateText`. History, typed context conversion, attachments and reply routing
are owned by the SDK through `ctx.message.history()`, `convertToAiSdkMessages`
and `convertToAiSdkTools(ctx.channel.current)`.

Build on its own with `pnpm --dir sdk/ts --filter @trytilde/dev-example-agent-1... build`.
