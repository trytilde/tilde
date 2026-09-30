# Agno adapter

Core history and delivery live in `trytilde`. This package converts typed context to
Agno messages and channel tools to Agno functions, bundles an invocation's run options in
`tilde_agno(ctx, agent)`, serves registry skills through `TildeSkills` and lets `tilde deploy`
register module-level agents and teams.

```python
import tilde
from agno.agent import Agent
from agno.models.openai import OpenAIChat
from agno.skills import LocalSkills
from tilde_agno import TildeSkills, convert_to_agno_messages, tilde_agno

INFERENCE = tilde.inference("default")
responder = Agent(
    model=OpenAIChat(
        id="gpt-4o-mini",
        base_url=INFERENCE.base_url,
        api_key=INFERENCE.api_key,
        http_client=INFERENCE.async_client(),
    ),
    instructions="Respond using the current channel's tools. Returned model text is private.",
    skills=TildeSkills([LocalSkills("skills")]),
    tool_call_limit=8,
    telemetry=False,
)


async def run(ctx):
    history = await ctx.message.history()
    messages = await convert_to_agno_messages(history.items, context=ctx)
    await responder.arun(input=messages, **await tilde_agno(ctx, responder))
```

`tilde_agno` returns `run_context` (with the current channel's tools as Agno's per-run
`client_tools`), a `run_id` that a stop or suspension cancels, and the thread as `session_id`.
Agno has no per-step message hook, so input steered while the agent works is appended to the
next Tilde tool result by a tool hook (an agent-level `tool_hooks` replaces it). Dynamic
`instructions`, `description` or `system_message` stamp the invocation's model calls.
`TildeSkills` is Agno's `Skills` plus the skills assigned to the agent in Tilde, loaded for each
invocation from `ctx.skills.directory()`; with plain `Skills` (or none) the registry skills come
as `list_skills`/`read_skill` tools with their summary in the system message.

`tilde deploy` (entry point `tilde.discover`) registers each module-level `Agent`/`Team`:
`<name or variable>/instructions` (`/<n>` from 1 for a list), `/description` and
`/system_message`, plain or braces when `{var}` is resolved in context, dynamic (the function's
source) for callables; `LocalSkills` folders ship with the deployment.

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

## Bundled tools

The agent's own Agno tools join Tilde's with one call:

```python
from tilde import BundledOptions
from tilde_agno import with_tilde_tools


def roll_dice(count: int = 1) -> list[int]:
    """Roll six-sided dice."""
    return [random.randint(1, 6) for _ in range(count)]


tools = await with_tilde_tools(
    ctx, [roll_dice], options={"roll_dice": BundledOptions(summary="Rolled dice")}
)
agent = Agent(model=model, tools=tools)
await agent.arun(input=messages)
```

`with_tilde_tools` takes plain functions, `@tool` functions and toolkits and returns the
current channel's tools, `ctx.agent_tools` and copies of the native functions. It publishes the
native tools to Tilde with their parameter schema and MCP `annotations` hints, so
`tools.search` finds them and a `tools.execute` naming one runs it here as the same agent. Every
call is audited once with the model's call id by `pre_hook`/`post_hook`, chained around the
tool's own hooks; the entrypoint is untouched, so Agno's argument injection keeps working. Agno
has no free metadata or output schema, so summaries come from `options`. Run with `arun`: the
audit hooks are async. Agno swallows hook errors, so a failed audit never fails the call, and
toolkit-level instructions are not carried over.
