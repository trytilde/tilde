# Example agent: OpenAI Agents SDK

Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent OpenAI Agents (Python)** (`a9b1e5d7-4c26-4f3a-9e80-7d5c2b4f6a07`) with a deployment token and starts it dialing in to the local gateway
beside the other SDK examples. Set `DEV_AGENTS=py-openai-agents` to start only this one.

A local development agent mirroring `sdk/ts/examples/vercel-ai-example-agent`, built on the Python SDK and the
OpenAI Agents SDK (`openai-agents`). `main.py` configures the Tilde host and OpenAI client;
`agent.py` generates replies with `Runner.run`. History, typed context conversion,
attachments and reply routing are owned by the SDK through `ctx.message.history()`,
`convert_to_openai_agents_messages` and `convert_to_openai_agents_tools(ctx.channel.current)`.

The model receives only `ctx.channel.current` tools and responds by calling them. Returned
model text remains private; visible responses are provider tool actions. Attachments are
downloaded through the scoped SDK and converted into `input_image` / `input_file` / text
parts by `trytilde-openai-agents`. Assign channels in Tilde normally. Logs go through the
standard `logging` module; the core routes them to Tilde. Failures expose only the stage and
HTTP status, never upstream response bodies. OpenAI trace export is disabled.

Environment:

- `OPENAI_API_KEY`, `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (required; the token comes
  from registering a Gateway deployment for the agent). The host dials out to Tilde and
  exposes no endpoint.
- `OPENAI_MODEL` (default `gpt-4o-mini`)

Run from `sdk/py` with `uv run --package example-agent-openai-agents python examples/example-agent-openai-agents/main.py`.
