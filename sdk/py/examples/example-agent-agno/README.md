# Example agent (Agno)

Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent Agno (Python)** (`e3f6c8a2-1b47-4d95-a6c3-9f0e2d8b5a08`) with a deployment token and starts it dialing in to the local gateway
beside the other SDK examples. Set `DEV_AGENTS=py-agno` to start only this one.

A local development agent mirroring `sdk/ts/examples/vercel-ai-example-agent`, built on Agno and the Tilde
Python SDK. Set `TILDE_GATEWAY_URL` and `TILDE_DEPLOYMENT_TOKEN` (the token comes
from registering a Gateway deployment for the agent), then run `python main.py` from this
directory (`OPENAI_MODEL` defaults to `gpt-4o-mini`). The host dials out to Tilde and exposes
no endpoint.

The model receives only the current channel's tools and the agent's skills, and responds by
calling tools. Returned model text remains private; visible responses are provider tool actions.
Attachments are downloaded through the scoped SDK and converted into Agno media or text by
`trytilde-agno`.

`main.py` configures the dial-in Tilde host (`run_connected_agent`); `agent.py` defines the Agno
`Agent` at module scope (capped at eight tool calls, telemetry disabled) and runs it per
invocation with `**await tilde_agno(ctx, responder)` (channel tools, steering, cancellation).
`skills/reply-style` ships with the deployment; `TildeSkills` adds the skills assigned to the
agent in Tilde on each invocation. `tilde deploy` registers `responder/instructions` and the
skill. History and typed context conversion go through `ctx.message.history()` and
`convert_to_agno_messages`. Logs go through the standard `logging` module, which the core routes
to Tilde.

Model calls go through Tilde's inference gateway with `tilde.inference(TILDE_INFERENCE)` (default
`default`, the alias the local registrar gives the example's OpenAI connection), so the process
never holds a provider key; the model is built once at import and each request carries the token
of the invocation running it.
