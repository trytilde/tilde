# LangChain / LangGraph adapter

Core history and delivery live in `trytilde`. This package converts typed
context to LangChain messages and channel tools to LangChain tools, following
the core/framework separation of the TypeScript Vercel AI SDK adapter.

```python
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI
from tilde_langchain import convert_to_langchain_messages, convert_to_langchain_tools

history = await ctx.message.history()
messages = await convert_to_langchain_messages(history.items, context=ctx)
agent = create_agent(
    ChatOpenAI(model="gpt-4o-mini"),
    tools=convert_to_langchain_tools(ctx.channel.current),
    system_prompt="Respond using the current channel's tools. Returned model text is private.",
)
await agent.ainvoke({"messages": messages}, config={"recursion_limit": 16})
```

`create_react_agent` from `langgraph.prebuilt` accepts the same tools and
messages. Tools are async only and run on the agent's event loop, so cancelling
the invocation cancels in-flight channel calls.

History pages are chronological; pass `before_message_id=history.next_page_token`
for older messages. The latest page includes the current objective unless it
already matches the latest received message. `include_objective=False` omits it.
`include_work=True` also reads current goals/tasks and requires `work.read`.
Only the acting agent's messages become `AIMessage`; everything else is a
`HumanMessage`. Every converted message carries the Tilde message id.

Images and PDFs are downloaded through `ctx.attachments.download` and embedded as
standard `image` / `file` content blocks (base64 with `mime_type`). Text files
include their real content. Unsupported binary formats get an explicit
attachment description; use `on_attachment` to parse them yourself. No private
URL or credential needs to be exposed to the model.

```python
from langchain_core.messages import HumanMessage
from tilde_langchain import MessageHandlers

history = await ctx.message.history(include_work=True)
messages = await convert_to_langchain_messages(
    history.items,
    context=ctx,
    on_message=MessageHandlers(
        goal=lambda item: HumanMessage(id=item.id, content=f"Our goal: {item.goal.objective}"),
        task=lambda item: None,  # Omit this type, or provide a different rendering.
    ),
    on_attachment=decode_attachment,  # async def decode_attachment(conversion) -> block | None
)
```

`MessageHandlers` supports `message`, `objective`, `goal` and `task`; each may
be sync or async. Supplied handlers take precedence over cached/default
rendering; returning `None` omits an item. Without an override the converter
renders all supported types and skips conversation messages whose status is
not `complete`. Completed, attachment-free conversions use the existing
per-agent cache in bounded batches (100 items or 1 MiB) as plain
`{"id", "role", "content"}` JSON. Files are hydrated afresh and are never
stored in the cache; objectives/goals/tasks remain live projections rather
than cached chat records. A cached representation is only reused when its id
and role match and it contains no `image`/`file` block.

## Channel tools

`convert_to_langchain_tools(ctx.channel.current)` preserves the provider's
descriptions and JSON schemas (passed to LangChain as a JSON-schema
`args_schema`) and forwards the model's tool-call id to Tilde's audited tool
execution. It does not publish the model's final text. The agent chooses the
provider tool and arguments, including routing fields required by that provider.

The tool-call id comes from the `ToolCall` dict LangGraph's `ToolNode` (and so
`create_agent`) passes to `tool.ainvoke`. Invoking a tool directly with plain
arguments has no call id; a random UUID is generated so the call is still
audited, but repeated executions are not deduplicated in that case.

Override tool instructions with `instructions={"sendMessage": "..."}`, or change
the channel tool's `description` before conversion. When combining namespaces,
use `prefix="slack_"` (or another prefix) to keep model tool names distinct.
Names are sanitized to `[a-zA-Z0-9_-]` (max 64 characters); collisions raise.

The core SDK exposes typed callable tools on `ctx.channel.slack`, `github`,
`agentmail`, `linq`, `whatsapp`, `telnyx_whatsapp` and `native`, plus
`ctx.channel.connections()` / `ctx.channel.for_connection(id)` when several
connections use one provider. `ctx.channel.current` never falls back to a
different connection when the inbound connection has no available tools.
