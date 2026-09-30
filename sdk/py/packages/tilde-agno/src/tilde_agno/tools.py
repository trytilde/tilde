"""Expose Tilde channel tools to Agno with provider schemas and audited tool-call ids intact, and
publish the agent's own Agno tools to Tilde as bundled tools."""

from __future__ import annotations

import asyncio
import copy
import inspect
import uuid
from collections.abc import Callable, Mapping, Sequence
from contextvars import ContextVar
from typing import Any

from agno.tools.function import Function, FunctionCall
from agno.tools.toolkit import Toolkit

from tilde import AgentContext, BundledOptions, ChannelTool, Tool, ToolAnnotations
from tilde._tools import ToolSpec, bundled_tool, model_tool_name, tool_spec

# The FunctionCall of the Tilde tool being executed; a routed tools.execute runs within it.
_FUNCTION_CALL: ContextVar[FunctionCall] = ContextVar("tilde_agno_function_call")


def convert_to_agno_tools(
    channels: Mapping[str, ChannelTool | Tool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[Function]:
    """Build Agno ``Function`` tools that keep provider schemas and execution ids.

    ``skip_entrypoint_processing`` stops Agno from replacing the provider JSON schema with one
    introspected from the entrypoint. Agno injects the running ``FunctionCall`` into an
    entrypoint parameter named ``fc``; its ``call_id`` is the model's tool-call id and is
    forwarded to Tilde's audited execution. A provider schema with a property named ``fc`` is
    rejected here because Agno would drop that argument before it reached the channel.
    ``instructions`` overrides descriptions by channel name; ``prefix`` namespaces model tool
    names when combining collections.
    """
    result: list[Function] = []
    names: set[str] = set()
    for name, channel in channels.items():
        key = model_tool_name(f"{prefix}{name}")
        if not key or key in names:
            raise ValueError("Conflicting or invalid model tool names")
        names.add(key)
        if "fc" in (channel.input_schema.get("properties") or {}):
            raise ValueError(
                f"{name}: Agno reserves the `fc` argument for its injected FunctionCall"
            )
        description = (
            instructions[name]
            if instructions is not None and name in instructions
            else channel.description
        )
        result.append(
            Function(
                name=key,
                description=description,
                parameters=copy.deepcopy(channel.input_schema),
                entrypoint=_entrypoint(channel),
                skip_entrypoint_processing=True,
            )
        )
    return result


def _entrypoint(channel: ChannelTool | Tool):
    async def execute(fc: FunctionCall, **arguments: Any) -> Any:
        token = _FUNCTION_CALL.set(fc)
        try:
            return await channel.execute(arguments, tool_call_id=fc.call_id or None)
        finally:
            _FUNCTION_CALL.reset(token)

    return execute


async def with_tilde_tools(
    ctx: AgentContext,
    native_tools: Sequence[Callable[..., Any] | Function | Toolkit],
    *,
    options: Mapping[str, BundledOptions] | None = None,
) -> list[Function]:
    """The current channel's tools, the agent's other Tilde tools and its own tools, as Agno
    ``Function``s for ``Agent(tools=...)``.

    ``native_tools`` are plain functions, ``@tool`` functions or toolkits (expanded to their
    functions). Each is published to Tilde with its name, description, parameter schema and MCP
    ``annotations`` hints, so ``tools.search`` finds it and a ``tools.execute`` naming it runs it
    here, and is returned as a copy whose calls are audited once with the model's call id by
    ``pre_hook``/``post_hook`` chained around the tool's own hooks. Agno has no free metadata,
    so the summary and display come from ``options[name]``. Call it once per invocation.

    Caveats: run the agent with ``arun``; the audit hooks are async, and Agno's sync path does
    not await them. Agno logs and swallows hook exceptions, so a failed audit never fails the
    call. Toolkit-level instructions are not carried over. Agno has no output schema.
    """
    functions = convert_to_agno_tools({**ctx.channel.current, **ctx.agent_tools})
    names = {function.name for function in functions}
    definitions = {}
    for native in functions_of(native_tools):
        if native.name in names:
            raise ValueError(f"Tool {native.name} conflicts with another tool")
        names.add(native.name)
        function = _audited(ctx, native)
        definitions[function.name] = bundled_tool(
            describe_tool(function, options), _route(function)
        )
        functions.append(function)
    # Published even when empty, so tools removed since the last call stop running here.
    await ctx._set_bundled_tools(definitions)
    return functions


def describe_tool(
    function: Function, options: Mapping[str, BundledOptions] | None = None
) -> ToolSpec:
    """What ``with_tilde_tools`` publishes and ``tilde deploy`` declares for one tool, from a
    ``Function`` whose entrypoint has been processed (see ``processed``)."""
    hints = function.annotations or {}
    annotations = (
        ToolAnnotations(
            read_only=bool(hints.get("readOnlyHint")),
            destructive=bool(hints.get("destructiveHint")),
            idempotent=bool(hints.get("idempotentHint")),
            open_world=bool(hints.get("openWorldHint")),
        )
        if hints
        else None
    )
    return tool_spec(
        function.name,
        function.description or "",
        function.parameters,
        annotations=annotations,
        options=(options or {}).get(function.name),
    )


def processed(native: Function) -> Function:
    """A copy with the parameters and description Agno introspects from the entrypoint."""
    function = native.model_copy(deep=True)
    function.process_entrypoint()
    return function


def functions_of(tools: Sequence[Callable[..., Any] | Function | Toolkit]) -> list[Function]:
    """Plain functions, ``@tool`` functions and toolkits (expanded) as unprocessed ``Function``s."""
    result: list[Function] = []
    for tool in tools:
        if isinstance(tool, Toolkit):
            result.extend(tool.get_async_functions().values())
        elif isinstance(tool, Function):
            result.append(tool)
        else:
            result.append(Function.from_callable(tool))
    return result


def _audited(ctx: AgentContext, native: Function) -> Function:
    # The entrypoint stays untouched so Agno still injects agent, run_context and media.
    function = processed(native)
    pre, post = native.pre_hook, native.post_hook

    async def pre_hook(fc: FunctionCall) -> None:
        fc.call_id = fc.call_id or str(uuid.uuid4())
        await ctx._report_bundled(function.name, fc.call_id, "running", input=fc.arguments)
        if pre is not None:
            await _call_hook(pre, fc)

    async def post_hook(fc: FunctionCall) -> None:
        # Agno runs the post hook in a finally block: an error means the call failed, and a
        # cancelled call is left unreported like any other cancellation.
        call = fc.call_id or ""
        task = asyncio.current_task()
        if task is not None and task.cancelling():
            pass
        elif fc.error is not None:
            await ctx._report_bundled(function.name, call, "failed", error=fc.error)
        else:
            await ctx._report_bundled(function.name, call, "completed", output=fc.result)
        if post is not None:
            await _call_hook(post, fc)

    function.pre_hook = pre_hook
    function.post_hook = post_hook
    return function


async def _call_hook(hook: Callable[..., Any], fc: FunctionCall) -> None:
    # The developer's own hook, with the arguments Agno would have injected by name.
    available = {
        "agent": fc.function._agent,
        "team": fc.function._team,
        "run_context": fc.function._run_context,
        "fc": fc,
    }
    parameters = inspect.signature(hook).parameters
    result = hook(**{name: value for name, value in available.items() if name in parameters})
    if inspect.isawaitable(result):
        await result


def _route(function: Function):
    # A tools.execute naming this tool runs it as Agno would, with the agent and run context of
    # the tools.execute call, audited once by its hooks.
    async def execute(input: Any, call: str) -> Any:
        outer = _FUNCTION_CALL.get(None)
        inner = function.model_copy(deep=True)
        if outer is not None:
            inner._agent = outer.function._agent
            inner._team = outer.function._team
            inner._run_context = outer.function._run_context
        fc = FunctionCall(function=inner, arguments=input, call_id=call)
        result = await fc.aexecute()
        if result.status == "failure":
            raise RuntimeError(result.error or f"{function.name} failed")
        return fc.result

    return execute
