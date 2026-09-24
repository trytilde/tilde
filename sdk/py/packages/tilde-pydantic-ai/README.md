# Pydantic AI adapter

Core history and delivery live in `trytilde`. This package converts typed context to
Pydantic AI `ModelMessage` history and channel tools to Pydantic AI tools, following the
core/framework separation of the TypeScript adapters.

```python
from pydantic_ai import Agent
from pydantic_ai.usage import UsageLimits
from tilde_pydantic_ai import convert_to_pydantic_ai_messages, convert_to_pydantic_ai_tools

history = await ctx.message.history()
messages = await convert_to_pydantic_ai_messages(history.items, context=ctx)
agent = Agent(
    model,
    instructions="Respond using the current channel's tools. Returned model text is private.",
    tools=convert_to_pydantic_ai_tools(ctx.channel.current),
)
await agent.run(message_history=messages, usage_limits=UsageLimits(request_limit=8))
```

History pages are chronological; pass `before_message_id=history.next_page_token` for older
messages. The latest page includes the current objective unless it already matches the latest
received message. `include_objective=False` omits it. `include_work=True` also reads current
goals/tasks and requires `work.read`. Only the acting agent's messages become `ModelResponse`
items; everything else is a `ModelRequest` with a `UserPromptPart`. Messages whose status is
not `complete` are skipped unless a custom `message` handler is given.

Pydantic AI parts have no id field. Each converted message keeps its Tilde id in the
application-level `metadata` (`{"tilde_message_id": ...}`, read it with `message_id(message)`);
that metadata is never sent to the model.

Images and PDFs are downloaded through `ctx.attachments.download` and embedded as
`BinaryContent`. Text files include their real content. Unsupported binary formats get an
explicit attachment description; use `on_attachment` to parse them yourself. No private URL or
credential needs to be exposed to the model. Converting a message with attachments without a
`context` or custom handler raises instead of silently dropping the file.

```python
from tilde_pydantic_ai import MessageHandlers, text_message

history = await ctx.message.history(include_work=True)
messages = await convert_to_pydantic_ai_messages(
    history.items,
    context=ctx,
    on_message=MessageHandlers(
        goal=lambda item: text_message(item.id, "user", f"Our goal: {item.goal.objective}"),
        task=lambda item: None,  # Omit this type, or provide a different rendering.
    ),
    on_attachment=decode_attachment,  # async def decode_attachment(input) -> str | BinaryContent
)
```

`MessageHandlers` supports `message`, `objective`, `goal` and `task`; handlers may be sync or
async. Supplied handlers take precedence over cached/default rendering; returning `None` omits
an item. Without an override the converter renders all supported types. Completed, attachment-free
conversation messages are cached through the existing per-agent cache in bounded batches (100
items or 1 MiB) as a small `{"id", "role", "text"}` JSON value; a cached entry is reused only
when its id and role match. Files are hydrated afresh and never stored in the cache;
objectives/goals/tasks remain live projections rather than cached chat records.

## Channel tools

`convert_to_pydantic_ai_tools(ctx.channel.current)` preserves the provider's descriptions and
JSON schemas (via `Tool.from_schema`) and forwards Pydantic AI's `RunContext.tool_call_id` to
Tilde's audited tool execution. It does not publish the model's final text. The agent chooses
the provider tool and arguments, including routing fields required by that provider.
Cancellation is enforced by the core: a stopped invocation refuses further tool RPCs.

Override tool instructions with `instructions={"sendMessage": "..."}`, or change the channel
tool's `description` before conversion. When combining namespaces, use `prefix="slack_"` (or
another prefix) to keep model tool names distinct. Names are sanitized to `[a-zA-Z0-9_-]` and
at most 64 characters; conflicts raise.

The core SDK exposes typed callable tools on `ctx.channel.slack`, `github`, `agentmail`, `linq`,
`whatsapp`, `telnyx_whatsapp` and `native`. Use `ctx.channel.connections()` and
`ctx.channel.for_connection(id)` when multiple connections use a provider. `ctx.channel.current`
never falls back to a different connection when the inbound connection has no available tools.

Cap a run with `usage_limits=UsageLimits(request_limit=8)` (or `tool_calls_limit`), the
Pydantic AI equivalent of `stepCountIs`.
