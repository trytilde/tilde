# Agno adapter

Core history and delivery live in `trytilde`. This package converts typed context to
Agno messages and channel tools to Agno functions, following Tilde's core/framework
separation.

```python
from agno.agent import Agent
from agno.models.openai import OpenAIChat
from tilde_agno import convert_to_agno_messages, convert_to_agno_tools

history = await ctx.message.history()
messages = await convert_to_agno_messages(history.items, context=ctx)
agent = Agent(
    model=OpenAIChat(id="gpt-4o-mini"),
    instructions="Respond using the current channel's tools. Returned model text is private.",
    tools=convert_to_agno_tools(ctx.channel.current),
    tool_call_limit=8,
    telemetry=False,
)
await agent.arun(input=messages)
```

The converted list is passed as the run input. Agno appends a `list[Message]` input verbatim
after its own system message, so roles and media survive and Tilde remains the source of
history; leave `add_history_to_context` off. `tool_call_limit` caps tool-call rounds: once
reached, further calls receive an error result instead of executing.

History pages are chronological; pass `before_message_id=history.next_page_token` for older
messages. The latest page includes the current objective unless it already matches the latest
received message. `include_objective=False` omits it. `include_work=True` also reads current
goals/tasks and requires `work.read`. Only the acting agent's messages receive the assistant
role.

Images and PDFs are downloaded through `ctx.attachments.download` and attached as Agno
`Image`/`File` media. Text files include their real content. Unsupported binary formats get an
explicit attachment description; use `on_attachment` to parse them yourself. No private URL or
credential needs to be exposed to the model.

```python
from agno.models.message import Message
from tilde_agno import MessageHandlers

history = await ctx.message.history(include_work=True)
messages = await convert_to_agno_messages(
    history.items,
    context=ctx,
    on_message=MessageHandlers(
        goal=lambda item: Message(
            id=item.id, role="user", content=f"Our goal: {item.goal.objective}"
        ),
        task=lambda item: None,  # Omit this type, or provide a different rendering.
    ),
    on_attachment=decode_your_format,  # (conversion) -> str | Image | File | list | None
)
```

`MessageHandlers` supports `message`, `objective`, `goal`, and `task`; handlers may be sync or
async. Supplied handlers take precedence over cached/default rendering; returning None omits
an item. Without an override the converter renders all supported types. Completed
conversation conversions use the existing per-agent cache, in bounded batches. Files are
hydrated afresh and are never stored in the cache; objectives/goals/tasks remain live
projections rather than cached chat records.

## Channel tools

`convert_to_agno_tools(ctx.channel.current)` preserves the provider's descriptions and JSON
schemas (`skip_entrypoint_processing` keeps Agno from introspecting a replacement schema) and
forwards the model's tool-call id to Tilde's audited tool execution. Agno injects the running
`FunctionCall` into the entrypoint's `fc` parameter; its `call_id` is the id the model chose.
The adapter does not publish the model's final text. The agent chooses the provider tool and
arguments, including routing fields required by that provider.

Override tool instructions with `instructions={"sendMessage": "..."}`, or change the channel
tool's `description` before conversion. When combining namespaces, use `prefix="slack_"` (or
another prefix) to keep model tool names distinct; names are sanitized to `[a-zA-Z0-9_-]` and
conflicts raise.

The core SDK exposes typed callable tools on `ctx.channel.slack`, `github`, `agentmail`,
`linq`, `whatsapp`, `telnyx_whatsapp`, and `native`. Use `ctx.channel.connections()` and
`ctx.channel.for_connection(id)` when multiple connections use a provider.
