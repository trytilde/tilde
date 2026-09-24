# Tilde Python SDK workspace

A uv workspace with the Python SDK packages and example agents:

- `packages/tilde` — `trytilde` (import `tilde`): dial-in agent host, invocation context, channel
  tools, message history, management/runtime clients, FastAPI integration and the chat proxy.
- `packages/tilde-langchain` — `trytilde-langchain`: LangChain / LangGraph messages and tools.
- `packages/tilde-pydantic-ai` — `trytilde-pydantic-ai`: Pydantic AI messages and tools.
- `packages/tilde-openai-agents` — `trytilde-openai-agents`: OpenAI Agents SDK items and tools.
- `packages/tilde-agno` — `trytilde-agno`: Agno messages and functions.
- `packages/tilde-crewai` — `trytilde-crewai`: CrewAI messages and tools. CrewAI pins
  `openai<3` and the OpenAI Agents SDK needs `openai>=3`, so the two are declared as uv
  conflicts and CrewAI is synced into its own environment, `.venv-crewai`.
- `examples/example-agent-*` — private example agents mirroring `sdk/ts/examples/vercel-ai-example-agent`.

```sh
task setup:py        # scripts/sync.sh: .venv plus .venv-crewai, both from uv.lock
task generate:py     # Python contracts from proto/ (not committed)
task test:sdk:py     # ruff + pytest for every package
task test:sdk:py:integration   # Python host against the real engine and disposable Postgres
```

Agents only dial out to Tilde (`RunService.Watch` with a deployment token); Tilde never calls an
agent over HTTP, so hosts expose no endpoint. Each package README documents its API. Package versions follow the shared Changie release
version like the TypeScript packages.
