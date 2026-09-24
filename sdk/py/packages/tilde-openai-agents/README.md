# OpenAI Agents SDK adapter

Core history and delivery live in `trytilde`. This package converts typed context to
OpenAI Agents SDK input items and channel tools to `FunctionTool`s, following the
core/framework separation of the TypeScript adapters.

```python
from agents import Agent, Runner, set_tracing_disabled
from tilde_openai_agents import convert_to_openai_agents_messages, convert_to_openai_agents_tools

set_tracing_disabled(True)  # or OPENAI_AGENTS_DISABLE_TRACING=1; keeps traces out of OpenAI

history = await ctx.message.history()
items = await convert_to_openai_agents_messages(history.items, context=ctx)
agent = Agent(
    name="assistant",
    instructions="Respond using the current channel's tools. Returned model text is private.",
    tools=convert_to_openai_agents_tools(ctx.channel.current),
)
await Runner.run(agent, items, max_turns=8)
```

History pages are chronological; pass `before_message_id=history.next_page_token` for
older messages. The latest page includes the current objective unless it already matches
the latest received message. `include_objective=False` omits it. `include_work=True` also
reads current goals/tasks and requires `work.read`. Only the acting agent's messages receive
the assistant role (`output_text` parts); everything else is a user item (`input_text`).

Returned items never carry an `id`: the Responses API rejects ids it did not issue. The
Tilde message id is stored only inside the cached representation, where it validates that
a cached rendering belongs to the message being hydrated.

Images and PDFs are downloaded through `ctx.attachments.download` and embedded as
`input_image` / `input_file` data URLs. Text files include their real content. Unsupported
binary formats get an explicit attachment description; use `on_attachment` to parse them
yourself. No private URL or credential needs to be exposed to the model. Without a context
or custom handler, a message with attachments raises instead of being silently truncated.

```python
history = await ctx.message.history(include_work=True)
items = await convert_to_openai_agents_messages(
    history.items,
    context=ctx,
    on_message=MessageHandlers(
        goal=lambda item: {
            "role": "user",
            "content": [{"type": "input_text", "text": f"Our goal: {item.goal.objective}"}],
        },
        task=lambda item: None,  # Omit this type, or provide a different rendering.
    ),
    on_attachment=decode_your_format,  # async def (AttachmentConversion) -> part | [parts] | None
)
```

`MessageHandlers` supports `message`, `objective`, `goal`, and `task`; handlers may be sync
or async. Supplied handlers take precedence over cached/default rendering; returning None
omits an item. Without an override the converter renders all supported types (incomplete
conversation messages are skipped unless a `message` handler is given). Completed
conversation conversions use the existing per-agent cache, in bounded batches (100 items or
1 MiB). Files are hydrated afresh and are never stored as data URLs in the cache;
objectives/goals/tasks remain live projections rather than cached chat records.

## Channel tools

`convert_to_openai_agents_tools(ctx.channel.current)` preserves the provider's descriptions
and JSON schemas (`strict_json_schema=False`, so provider schemas are used unchanged) and
forwards the Agents SDK tool-call id (`ToolContext.tool_call_id`) to Tilde's audited tool
execution. Provider results are returned to the model as JSON strings. The adapter does
not publish the model's final text. The agent chooses the provider tool and arguments,
including routing fields required by that provider.

Override tool instructions with `instructions={"sendMessage": "..."}`, or change the
channel tool's `description` before conversion. When combining collections, use
`prefix="slack_"` (or another prefix) to keep model tool names distinct; names are limited
to `[a-zA-Z0-9_-]` and 64 characters, and conflicts raise.

The core SDK exposes callable tools on `ctx.channel.slack`, `github`, `agentmail`, `linq`,
`whatsapp`, `telnyx_whatsapp`, and `native`. Use `ctx.channel.connections()` and
`ctx.channel.for_connection(id)` when multiple connections use a provider.
`ctx.channel.current` never falls back to a different connection.
