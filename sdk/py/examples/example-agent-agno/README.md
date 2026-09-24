# Example agent (Agno)

Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent Agno (Python)** (`e3f6c8a2-1b47-4d95-a6c3-9f0e2d8b5a08`) with a deployment token and starts it dialing in to the local gateway
beside the other SDK examples. Set `DEV_AGENTS=py-agno` to start only this one.

A local development agent mirroring `sdk/ts/examples/vercel-ai-example-agent`, built on Agno and the Tilde
Python SDK. Set `OPENAI_API_KEY`, `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (the token comes
from registering a Gateway deployment for the agent), then run `python main.py` from this
directory (`OPENAI_MODEL` defaults to `gpt-4o-mini`). The host dials out to Tilde and exposes
no endpoint.

The model receives only `ctx.channel.current` tools and responds by calling them. Returned
model text remains private; visible responses are provider tool actions. Attachments are
downloaded through the scoped SDK and converted into Agno media or text by `trytilde-agno`.

`main.py` configures the dial-in Tilde host (`run_connected_agent`) and the Agno OpenAI model; `agent.py` generates
replies with `Agent.arun`, capped at eight tool calls, with Agno telemetry disabled. History,
typed context conversion, attachments and reply routing are owned by the SDK through
`ctx.message.history()`, `convert_to_agno_messages` and
`convert_to_agno_tools(ctx.channel.current)`. Logs go through the standard `logging` module,
which the core routes to Tilde.
