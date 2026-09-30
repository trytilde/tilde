# LangChain / LangGraph adapter

Core history and delivery live in `trytilde`. This package converts typed
context to LangChain messages and channel tools to LangChain tools, brings each
invocation to a module-level `create_agent` graph through `tilde_middleware()`,
and lets `tilde deploy` discover the agent's prompts.

```python
import tilde
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI
from tilde_langchain import convert_to_langchain_messages, tilde_middleware

INFERENCE = tilde.inference("default")
agent = create_agent(
    ChatOpenAI(
        model="gpt-4o-mini",
        base_url=INFERENCE.base_url,
        api_key=INFERENCE.api_key,
        http_async_client=INFERENCE.async_client(),
    ),
    system_prompt="Respond using the current channel's tools. Returned model text is private.",
    middleware=[tilde_middleware()],
)


async def run(ctx):
    history = await ctx.message.history()
    messages = await convert_to_langchain_messages(history.items, context=ctx)
    await agent.ainvoke({"messages": messages}, {"recursion_limit": 16})
```

A compiled graph takes no per-call tools or messages (`ainvoke`'s config carries
callbacks and `context`, and callbacks cannot change state), so the invocation's
pieces come from middleware the graph is created with once. `tilde_middleware()`
reads the running invocation from Tilde's context variable and does nothing
outside one:

- before each model call it checks cancellation and adds steering input as user
  messages to the graph state, so later calls keep them;
- around each model call it adds the current channel's tools and, when skills are
  assigned (plain `create_agent` has no native skills), the `list_skills` /
  `read_skill` tools plus `ctx.skills.summary()` as a second system message;
  it also stamps the invocation's inference calls with every `@dynamic_prompt`
  listed after it, so put it first;
- around tool calls it executes those tools. They are not registered with the
  graph's `ToolNode`; `create_agent` allows that when middleware executes them
  in `wrap_tool_call`.

The hooks are async only; use `ainvoke` / `astream`.

## Deploy discovery

`tilde deploy` (entry point `tilde.discover`) registers:

- a module-level `create_agent` graph's static `system_prompt` as the plain prompt
  `<name>/system_prompt` (`create_agent(name=...)`, else the variable name), read
  from the model node's closure (langchain 1.4; a warning when unreadable), and every
  `@dynamic_prompt` middleware in the graph;
- a `@dynamic_prompt` middleware as the dynamic prompt `<function>/system_prompt`
  with the function's source;
- module-level `PromptTemplate`s as `<variable>` and `ChatPromptTemplate` messages as
  `<variable>/<index>.<role>`: f-string templates in braces format, mustache in
  mustache format. Jinja2 templates and LangSmith hub prompts are reported, not
  registered.

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

`tilde_middleware()` converts the channel's tools itself; convert them yourself for
graphs built per invocation or `create_react_agent`. Tools are async only and run on
the agent's event loop, so cancelling the invocation cancels in-flight channel calls.

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
Names are sanitized to `[a-zA-Z0-9_-]`, and names over 64 characters are truncated with a
stable hash suffix; collisions raise.

The core SDK exposes typed callable tools on `ctx.channel.slack`, `github`,
`agentmail`, `linq`, `whatsapp`, `telnyx_whatsapp` and `native`, plus
`ctx.channel.connections()` / `ctx.channel.for_connection(id)` when several
connections use one provider. `ctx.channel.current` never falls back to a
different connection when the inbound connection has no available tools.

## Bundled tools

The agent's own LangChain tools join Tilde's with one call:

```python
from langchain_core.tools import tool
from tilde import BundledOptions
from tilde_langchain import with_tilde_tools


@tool
def roll_dice(count: int = 1) -> list[int]:
    """Roll six-sided dice."""
    return [random.randint(1, 6) for _ in range(count)]


roll_dice.metadata = {"tilde": BundledOptions(summary="Rolled dice")}
agent = create_agent(model, tools=await with_tilde_tools(ctx, [roll_dice]))
```

`with_tilde_tools` returns the current channel's tools, `ctx.agent_tools` and copies of the
native tools, and publishes the native tools to Tilde with the argument schema the model sees,
so `tools.search` finds them and a `tools.execute` naming one runs it here. Each copy keeps its
type and gets an audit callback handler, so `ToolNode` injection keeps working and every call
is audited once with the model's call id. `options={"roll_dice": BundledOptions(...)}` overrides
`metadata["tilde"]`. LangChain has no output schema and audits record the tool message content;
a `ToolException` handled for the model is audited as failed. Tools that need `ToolRuntime` or
`InjectedState` only get them from `ToolNode`, so they cannot run through `tools.execute`.
