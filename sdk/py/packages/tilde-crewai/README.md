# CrewAI adapter

Core history and delivery live in `trytilde`. This package converts typed context to
CrewAI messages and channel tools to CrewAI tools, following Tilde's core/framework
separation.

```python
from crewai import LLM, Agent
from tilde_crewai import convert_to_crewai_messages, convert_to_crewai_tools

history = await ctx.message.history()
messages = await convert_to_crewai_messages(history.items, context=ctx)
agent = Agent(
    role="Support assistant",
    goal="Respond using the current channel's tools.",
    backstory="Returned model text is private; visible replies are channel tool calls.",
    llm=LLM(model="openai/gpt-4o-mini"),
    tools=convert_to_crewai_tools(ctx.channel.current),
    max_iter=8,
)
await agent.kickoff_async(messages)
```

Set `CREWAI_DISABLE_TELEMETRY=true` in the environment before `crewai` is imported to turn
off CrewAI's anonymous telemetry. `OTEL_SDK_DISABLED=true` has the same effect but also
disables your own OpenTelemetry SDK, so prefer the CrewAI flag. `max_iter` caps the agent's
model/tool iterations. Leave agent `memory` off; Tilde owns the history.

The converted list is passed to `kickoff_async`. CrewAI messages are OpenAI-style
`{"role", "content"}` dicts (`crewai.utilities.types.LLMMessage`) and carry no id. Every message
keeps its role as conversation history, except the last `user` message: CrewAI collapses it to
text and promotes it into its task prompt (`Current Task: ...`). Content parts on that message
would be dropped, so when it carries media the converter appends a short text request
(`tilde_crewai.messages.MEDIA_REQUEST`) to be promoted in its place.

History pages are chronological; pass `before_message_id=history.next_page_token` for older
messages. The latest page includes the current objective unless it already matches the latest
received message. `include_objective=False` omits it. `include_work=True` also reads current
goals/tasks and requires `work.read`. Only the acting agent's messages receive the assistant
role.

Images and PDFs are downloaded through `ctx.attachments.download` and attached as content
parts with base64 data URLs. Text files include their real content. Unsupported binary formats
get an explicit attachment description; use `on_attachment` to parse them yourself. No private
URL or credential needs to be exposed to the model. CrewAI has no media type for history and
passes content parts to the provider unchanged, so the default parts are the OpenAI Chat
Completions shapes (`image_url`, `file`), CrewAI's default OpenAI API. For the Responses API
or another provider, return that provider's part from `on_attachment`. CrewAI's own `files`
message field is not used: it needs the optional `crewai-files` extra.

```python
from tilde_crewai import MessageHandlers

history = await ctx.message.history(include_work=True)
messages = await convert_to_crewai_messages(
    history.items,
    context=ctx,
    on_message=MessageHandlers(
        goal=lambda item: {"role": "user", "content": f"Our goal: {item.goal.objective}"},
        task=lambda item: None,  # Omit this type, or provide a different rendering.
    ),
    on_attachment=decode_your_format,  # (conversion) -> str | content part dict | list | None
)
```

`MessageHandlers` supports `message`, `objective`, `goal`, and `task`; handlers may be sync or
async. Supplied handlers take precedence over cached/default rendering; returning None omits
an item. Without an override the converter renders all supported types. Completed
conversation conversions use the existing per-agent cache, in bounded batches. Files are
hydrated afresh and are never stored in the cache; objectives/goals/tasks remain live
projections rather than cached chat records.

## Channel tools

`convert_to_crewai_tools(ctx.channel.current)` returns `crewai.tools.BaseTool` instances that
keep the provider's descriptions and JSON schemas. CrewAI reads tool parameters from a pydantic
`args_schema`; the adapter supplies a field-less model whose `model_json_schema()` returns the
provider schema, so nothing is introspected and arguments reach the channel exactly as the
model sent them. The adapter does not publish the model's final text. The agent chooses the
provider tool and arguments, including routing fields required by that provider.

CrewAI itself changes three things that an adapter cannot turn off:

- Every tool schema goes through its OpenAI strict-mode pass before it reaches the model:
  all properties of every object become `required`, objects get
  `additionalProperties: false`, `oneOf` becomes `anyOf`, `$ref`s are inlined and unsupported
  `format`s are removed. Types, nesting, enums and descriptions are preserved. The model must
  therefore supply a value for optional provider fields.
- Model-facing names are lowercased snake_case (`sendMessage` becomes `send_message`).
  Conflicts are checked on that name.
- The model's tool-call id is not passed to tools, hooks or events, so it cannot be forwarded.
  Tilde's core generates a UUID per execution; audited tool-call ids do not match the
  provider's ids.

CrewAI executes tools synchronously on worker threads. Channel execution belongs to the
invocation's event loop, so call `convert_to_crewai_tools` on that loop (inside your `run`
handler): the tools capture it and submit each execution back to it, blocking only the worker
thread. `await tool.arun(...)` runs directly on the loop. Results are returned to the model as
JSON. CrewAI's tool-result cache is opt-in; leave it off so repeated sends are executed.

Override tool instructions with `instructions={"sendMessage": "..."}`, or change the channel
tool's `description` before conversion. When combining namespaces, use `prefix="slack_"` (or
another prefix) to keep model tool names distinct; names are sanitized to `[a-zA-Z0-9_-]` and
conflicts raise.

The core SDK exposes typed callable tools on `ctx.channel.slack`, `github`, `agentmail`,
`linq`, `whatsapp`, `telnyx_whatsapp`, and `native`. Use `ctx.channel.connections()` and
`ctx.channel.for_connection(id)` when multiple connections use a provider.
