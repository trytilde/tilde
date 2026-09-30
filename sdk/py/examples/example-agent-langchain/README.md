Run with `REGISTER_DEV_AGENTS=1 task dev`. The debug-only `tilde dev-agent` command registers
**Example Agent LangChain (Python)** (`7e4a2b91-0c5d-4e86-b3f7-1a9d5c6e8f05`) with a deployment token and starts it dialing in to the local gateway
beside the other SDK examples. Set `DEV_AGENTS=py-langchain` to start only this one.

Example agent built on the Tilde Python SDK and LangChain 1.x. It mirrors
`sdk/ts/examples/vercel-ai-example-agent` (TypeScript, Vercel AI SDK): the model receives only
`ctx.channel.current` tools and responds by calling them. Returned model text
remains private; visible responses are provider tool actions. Attachments are
downloaded through the scoped SDK and converted into image/file/text content
blocks by `trytilde-langchain`.

Run it from `sdk/py` with the workspace environment:

```
TILDE_GATEWAY_URL=... TILDE_DEPLOYMENT_TOKEN=... uv run --directory examples/example-agent-langchain python main.py
```

`OPENAI_MODEL` (default `gpt-4o-mini`) is optional. The deployment token comes
from registering a Gateway deployment for the agent; the host dials out to Tilde
and exposes no endpoint. Assign channels normally.

`main.py` configures the dial-in Tilde host (`run_connected_agent`). `agent.py`
creates the agent once at module scope with `create_agent`, the static system
prompt and `tilde_middleware()`, and per invocation calls `agent.ainvoke` with the
converted history and a recursion limit of 16. The middleware gives each
invocation the current channel's tools, steering input, dynamic prompt stamps and
the registry's skills (as `list_skills` / `read_skill` tools plus a summary; plain
`create_agent` has no native skills). History, typed context conversion,
attachments and reply routing are owned by the SDK through `ctx.message.history()`
and `convert_to_langchain_messages`. Logging goes through the standard `logging`
module, which the core SDK routes to Tilde per invocation. Failures expose only the
stage and HTTP status, never upstream response bodies.

Model calls go through Tilde's inference gateway with `tilde.inference(TILDE_INFERENCE)`
(default `default`, the alias the local registrar gives the example's OpenAI connection), so
the process never holds a provider key; the model is built once and each request carries the
running invocation's token.

The system prompt is registered with the deployment as `example-agent-langchain/system_prompt`.
Preview it from this directory with `../../.venv/bin/python -m tilde deploy main.py --dry-run`;
`tilde dev` does the same when it registers the example.
