# Vercel AI SDK / LangSmith demo

A small recipe agent for inspecting LangSmith's trace UI. It uses AI SDK 6,
`@ai-sdk/openai`, Zod, and LangSmith's `wrapAISDK` integration. A run contains a
parent `dummy.recipe-agent` trace, an OpenAI tool-selection step, a deterministic
`getPantry` tool, and a streamed recipe response. The default model is
`gpt-4o-mini`; the LangSmith project defaults to `test`.

## Generate a trace without starting Tilde

From the repository root:

```sh
pnpm --dir sdk/ts install --frozen-lockfile
export LANGSMITH_API_KEY='your-key'
task agent:langsmith
```

Alternatively put `LANGSMITH_API_KEY` in the ignored root `.env.langsmith` file.
The runner extracts only `openai_api_key` from `secrets.enc.yaml` using SOPS and
passes it to Node as `OPENAI_API_KEY`. It never prints the key or writes a decrypted
key file. The current AWS session must have access to the SOPS KMS recipient.

The recipe streams to the terminal. The command waits for pending trace batches
and prints a direct LangSmith trace URL. Open that URL to inspect the run tree,
model inputs/outputs, tool arguments/results, timing and token usage.

Optional environment settings:

```sh
LANGSMITH_TRACING=true
LANGSMITH_ENDPOINT=https://api.smith.langchain.com
LANGSMITH_PROJECT=test
# LANGSMITH_WORKSPACE_ID=...  # If required by a multi-workspace API key.
OPENAI_MODEL=gpt-4o-mini
# AGENT_PROMPT=Write a vegetarian lasagna recipe for 4 people.
```

## Host it as a Tilde agent

```sh
export AGENT_SIGNING_KEY='at-least-32-characters-matching-registration'
task agent:langsmith:serve
```

Register `http://127.0.0.1:3002` using the same signing key and grant the agent
`tools.invoke` for `sendMessage`. Invoke it with a recipe objective; it streams the
answer through Tilde's native message tool. `AGENT_PORT` overrides the port.

LangSmith's native wrapper sends these demo traces directly to LangSmith. The
AI SDK's OTel instrumentation is also enabled, allowing Tilde's agent tracing
processor to collect model spans when hosted through `createAgentServer`. This
demo does not change the platform's external collector configuration.

## Local checks

```sh
task agent:langsmith:build
python3 scripts/run-langsmith-agent.py --check
```

The second command verifies SOPS access and reports whether a LangSmith key is
present without displaying either credential. Live generation requires both keys.
