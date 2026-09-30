# OpenAI Agents SDK adapter

Core history and delivery live in `trytilde`. This package converts typed context to
OpenAI Agents SDK input items and channel tools to `FunctionTool`s, bundles each invocation's
run options (`tilde_openai_agents`) and lets `tilde deploy` discover module-level agents.

```python
import tilde
from agents import Agent, OpenAIResponsesModel, Runner, set_tracing_disabled
from openai import AsyncOpenAI
from tilde_openai_agents import convert_to_openai_agents_messages, tilde_openai_agents

set_tracing_disabled(True)  # or OPENAI_AGENTS_DISABLE_TRACING=1; keeps traces out of OpenAI
INFERENCE = tilde.inference("default")
client = AsyncOpenAI(
    base_url=INFERENCE.base_url, api_key=INFERENCE.api_key, http_client=INFERENCE.async_client()
)
agent = Agent(
    name="assistant",
    instructions="Respond using the current channel's tools. Returned model text is private.",
    model=OpenAIResponsesModel(model="gpt-4o-mini", openai_client=client),
)


async def run(ctx):
    history = await ctx.message.history()
    items = await convert_to_openai_agents_messages(history.items, context=ctx)
    run_agent, run_config = await tilde_openai_agents(ctx, agent)
    await Runner.run(run_agent, items, run_config=run_config, max_turns=8)
```

`tilde_openai_agents(ctx, agent, run_config=None)` returns a clone of `agent` and a
`RunConfig` (a copy of yours, when given):

- the clone has the current channel's tools;
- skills assigned in the registry: with a local `ShellTool` they join its environment
  `skills` (written to disk by `ctx.skills.directory()`); otherwise the clone gets the
  `list_skills` / `read_skill` tools and `ctx.skills.summary()` after its instructions;
- the config's `call_model_input_filter` (chained after yours) checks cancellation before
  every model call, stamps the invocation's inference calls with the calling agent's dynamic
  instructions and adds steering input as user messages. The SDK sends filtered input for
  one call only, so steered messages are re-inserted where they arrived on later calls; with
  server-managed conversations (`previous_response_id`, `conversation_id`) that duplicates
  them.

Handoff targets and agent tools run as defined; only the returned agent has channel tools.

## Deploy discovery

`tilde deploy` (entry point `tilde.discover`) registers every module-level `Agent` and the
agents reachable through its handoffs and `as_tool` tools (cycles are fine):
`<name>/instructions` (a string is plain, a function dynamic with its source) and
`<name>/handoff_description`. Skill folders the SDK loads from disk are shipped: local
`ShellTool` environment skills and a `SandboxAgent`'s `Skills(from_=LocalDir(...))` or
`lazy_from=LocalDirLazySkillSource(...)`, resolved from the working directory. Hosted
prompts (`prompt={"id": ...}`) are versioned by OpenAI and reported as warnings. Handoff
targets and agent-tool agents are read from private fields (openai-agents 0.22); an
unreadable one is reported.

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
`prefix="slack_"` (or another prefix) to keep model tool names distinct; names are sanitized
to `[a-zA-Z0-9_-]`, truncated to 64 characters with a stable hash suffix, and conflicts raise.

The core SDK exposes callable tools on `ctx.channel.slack`, `github`, `agentmail`, `linq`,
`whatsapp`, `telnyx_whatsapp`, and `native`. Use `ctx.channel.connections()` and
`ctx.channel.for_connection(id)` when multiple connections use a provider.
`ctx.channel.current` never falls back to a different connection.

## Bundled tools

The agent's own function tools join Tilde's with one call:

```python
from agents import Agent, function_tool
from tilde import BundledOptions
from tilde_openai_agents import with_tilde_tools


@function_tool
def roll_dice(count: int = 1) -> list[int]:
    """Roll six-sided dice."""
    return [random.randint(1, 6) for _ in range(count)]


tools = await with_tilde_tools(
    ctx, [roll_dice], options={"roll_dice": BundledOptions(summary="Rolled dice")}
)
agent = Agent(name="assistant", tools=tools)
```

`with_tilde_tools` returns the current channel's tools, `ctx.agent_tools` and copies of the
native `FunctionTool`s with an audited `on_invoke_tool`, and publishes the native tools to
Tilde with their parameter and output schemas, so `tools.search` finds them and a
`tools.execute` naming one runs it here with the run's context. Every call is audited once with
the model's call id. `FunctionTool` has no free metadata, so summaries come from `options`.
`@function_tool` turns an exception into an error string for the model by default, so such a
failure is audited as completed with that text; `failure_error_function=None` audits it as
failed (and fails the run).
