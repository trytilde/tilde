# Example agent: OpenAI Agents SDK

Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent OpenAI Agents (Python)** (`a9b1e5d7-4c26-4f3a-9e80-7d5c2b4f6a07`) with a deployment token and starts it dialing in to the local gateway
beside the other SDK examples. Set `DEV_AGENTS=py-openai-agents` to start only this one.

A local development agent mirroring `sdk/ts/examples/vercel-ai-example-agent`, built on the Python SDK and the
OpenAI Agents SDK (`openai-agents`). `main.py` configures the Tilde host; `agent.py` defines
the `Agent` once at module scope and per invocation runs it with
`run_agent, run_config = await tilde_openai_agents(ctx, agent)`: the helper returns a clone
carrying the current channel's tools (and the registry's skills as `list_skills` /
`read_skill` tools plus a summary) and a `RunConfig` that checks cancellation, stamps dynamic
instructions and adds steering input before every model call. History, typed context
conversion, attachments and reply routing are owned by the SDK through
`ctx.message.history()` and `convert_to_openai_agents_messages`.

The model receives only `ctx.channel.current` tools and responds by calling them. Returned
model text remains private; visible responses are provider tool actions. Attachments are
downloaded through the scoped SDK and converted into `input_image` / `input_file` / text
parts by `trytilde-openai-agents`. Assign channels in Tilde normally. Logs go through the
standard `logging` module; the core routes them to Tilde. Failures expose only the stage and
HTTP status, never upstream response bodies. OpenAI trace export is disabled.

Environment:

- `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (required; the token comes
  from registering a Gateway deployment for the agent). The host dials out to Tilde and
  exposes no endpoint.
- `OPENAI_MODEL` (default `gpt-5.6-terra`)

Run from `sdk/py` with `uv run --package example-agent-openai-agents python examples/example-agent-openai-agents/main.py`.

Model calls go through Tilde's inference gateway with `tilde.inference(TILDE_INFERENCE)`
(default `default`, the alias the local registrar gives the example's OpenAI connection), so
the process never holds a provider key; the client is built once and each request carries the
running invocation's token.

The instructions are registered with the deployment as
`example-agent-openai-agents/instructions`. Preview them from this directory with
`../../.venv/bin/python -m tilde deploy main.py --dry-run`; `tilde dev` does the same when it
registers the example.
