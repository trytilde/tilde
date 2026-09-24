# Example agent: Pydantic AI

Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent Pydantic AI (Python)** (`2d8c7f63-9e14-4a0b-8d52-6b3f1e7a9c06`) with a deployment token and starts it dialing in to the local gateway
beside the other SDK examples. Set `DEV_AGENTS=py-pydantic-ai` to start only this one.

A local development agent mirroring `sdk/ts/examples/vercel-ai-example-agent`, written in Python. `main.py`
configures the dial-in Tilde host (`run_connected_agent`, no inbound endpoint) and
the OpenAI Responses model; `agent.py` generates replies with a Pydantic AI `Agent`.

Required environment: `OPENAI_API_KEY`, `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (the
token comes from registering a Gateway deployment for the agent). Optional: `OPENAI_MODEL`
(default `gpt-4o-mini`).

```
cd sdk/py
uv run --package example-agent-pydantic-ai python examples/example-agent-pydantic-ai/main.py
```

The model receives only `ctx.channel.current` tools and responds by calling them. Returned
model text remains private; visible responses are provider tool actions. History, typed
context conversion, attachments and reply routing are owned by the SDK through
`ctx.message.history()`, `convert_to_pydantic_ai_messages` and
`convert_to_pydantic_ai_tools(ctx.channel.current)`. Attachments are downloaded through the
scoped SDK and become `BinaryContent` (images, PDFs) or inline text. Runs are capped at 8 model
requests and 60 seconds. Logs go through the standard `logging` module, which the core routes
to Tilde per invocation. Failures expose only the stage and HTTP status, never upstream bodies.
