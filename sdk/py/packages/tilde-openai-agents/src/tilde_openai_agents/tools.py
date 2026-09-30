"""Channel tools as ``agents.FunctionTool`` instances with audited, id-preserving execution, and
the agent's own function tools published to Tilde as bundled tools."""

from __future__ import annotations

import json
from collections.abc import Awaitable, Callable, Mapping, Sequence
from contextvars import ContextVar
from dataclasses import replace
from typing import Any

from agents import FunctionTool
from agents.tool_context import ToolContext

from tilde import AgentContext, BundledOptions, ChannelTool, Tool
from tilde._tools import ToolSpec, bundled_tool, model_tool_name, tool_spec

# The ToolContext of the Tilde tool being executed; a routed tools.execute runs within it.
_TOOL_CONTEXT: ContextVar[ToolContext[Any]] = ContextVar("tilde_openai_agents_tool_context")


def convert_to_openai_agents_tools(
    channels: Mapping[str, ChannelTool | Tool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[FunctionTool]:
    """Preserve provider descriptions and schemas; forward the model's call id to Tilde.

    ``instructions`` overrides descriptions by channel tool name without touching schemas or
    authorization. ``prefix`` namespaces model tool names when combining collections.
    ``ctx.skills.tools()`` converts the same way.
    """
    tools: list[FunctionTool] = []
    names: set[str] = set()
    for name, channel in channels.items():
        key = model_tool_name(f"{prefix}{name}")
        if not key or key in names:
            raise ValueError("Conflicting or invalid model tool names")
        names.add(key)
        description = (
            instructions[name] if instructions and name in instructions else channel.description
        )
        tools.append(
            FunctionTool(
                name=key,
                description=description,
                params_json_schema=channel.input_schema,
                on_invoke_tool=_invoker(channel),
                strict_json_schema=False,
            )
        )
    return tools


def _invoker(channel: ChannelTool | Tool):
    async def on_invoke_tool(ctx: ToolContext, input_json: str) -> str:
        arguments = json.loads(input_json) if input_json.strip() else {}
        token = _TOOL_CONTEXT.set(ctx)
        try:
            result = await channel.execute(arguments, tool_call_id=ctx.tool_call_id)
        finally:
            _TOOL_CONTEXT.reset(token)
        return json.dumps(result, default=str)

    return on_invoke_tool


async def with_tilde_tools(
    ctx: AgentContext,
    native_tools: Sequence[FunctionTool],
    *,
    options: Mapping[str, BundledOptions] | None = None,
) -> list[FunctionTool]:
    """The current channel's tools, the agent's other Tilde tools and its own function tools,
    for ``Agent(tools=...)``.

    Each native tool (``@function_tool`` or ``FunctionTool``) is published to Tilde with its
    name, description, parameter and output schemas, so ``tools.search`` finds it and a
    ``tools.execute`` naming it runs it here, and is returned as a copy whose calls are audited
    once with the model's call id. ``FunctionTool`` has no free metadata, so the summary and
    display come from ``options[name]``. Call it once per invocation.

    Caveat: ``@function_tool`` turns a raised exception into an error string for the model by
    default (``failure_error_function``), so such a failure is audited as completed with that
    text; pass ``failure_error_function=None`` to have it audited as failed (it then fails the
    run).
    """
    tools = convert_to_openai_agents_tools({**ctx.channel.current, **ctx.agent_tools})
    names = {tool.name for tool in tools}
    definitions = {}
    for tool in native_tools:
        if tool.name in names:
            raise ValueError(f"Tool {tool.name} conflicts with another tool")
        names.add(tool.name)
        audited = replace(tool, on_invoke_tool=_audited(ctx, tool.name, tool.on_invoke_tool))
        definitions[tool.name] = bundled_tool(describe_tool(tool, options), _route(audited))
        tools.append(audited)
    # Published even when empty, so tools removed since the last call stop running here.
    await ctx._set_bundled_tools(definitions)
    return tools


def describe_tool(
    tool: FunctionTool, options: Mapping[str, BundledOptions] | None = None
) -> ToolSpec:
    """What ``with_tilde_tools`` publishes and ``tilde deploy`` declares for one tool."""
    return tool_spec(
        tool.name,
        tool.description,
        tool.params_json_schema,
        output_schema=tool.output_json_schema,
        options=(options or {}).get(tool.name),
    )


def _audited(
    ctx: AgentContext, name: str, invoke: Callable[[ToolContext[Any], str], Awaitable[Any]]
):
    async def on_invoke_tool(tool_ctx: ToolContext[Any], input_json: str) -> Any:
        arguments = json.loads(input_json) if input_json.strip() else {}
        return await ctx._run_audited(
            name, tool_ctx.tool_call_id, arguments, lambda: invoke(tool_ctx, input_json)
        )

    return on_invoke_tool


def _route(tool: FunctionTool):
    # A tools.execute naming this tool runs it as the Agents SDK would, with the run context,
    # usage and agent of the tools.execute call, audited once by its on_invoke_tool.
    async def execute(input: Any, call: str) -> Any:
        outer = _TOOL_CONTEXT.get(None)
        arguments = json.dumps(input)
        tool_ctx = (
            ToolContext(
                outer.context,
                outer.usage,
                tool_name=tool.name,
                tool_call_id=call,
                tool_arguments=arguments,
                agent=outer.agent,
                run_config=outer.run_config,
            )
            if outer is not None
            else ToolContext(None, tool_name=tool.name, tool_call_id=call, tool_arguments=arguments)
        )
        return await tool.on_invoke_tool(tool_ctx, arguments)

    return execute
