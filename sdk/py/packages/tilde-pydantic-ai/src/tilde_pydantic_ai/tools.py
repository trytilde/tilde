"""Expose Tilde channel tools to Pydantic AI without altering provider schemas or authorization,
and publish the agent's own Pydantic AI tools to Tilde as bundled tools."""

from __future__ import annotations

import inspect
import uuid
from collections.abc import Callable, Mapping, Sequence
from contextvars import ContextVar
from dataclasses import dataclass, replace
from typing import Any

from pydantic_ai import RunContext
from pydantic_ai.tools import Tool
from pydantic_ai.toolsets import AbstractToolset, FunctionToolset, WrapperToolset
from pydantic_ai.toolsets.abstract import ToolsetTool

from tilde import AgentContext, BundledOptions, ChannelTool
from tilde import Tool as TildeTool
from tilde._tools import ToolSpec, bundled_tool, model_tool_name, tool_spec

# The RunContext of the Tilde tool being executed; a routed tools.execute runs within it.
_RUN_CONTEXT: ContextVar[RunContext[Any]] = ContextVar("tilde_pydantic_ai_run_context")


def convert_to_pydantic_ai_tools(
    channels: Mapping[str, ChannelTool | TildeTool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[Tool[Any]]:
    """Preserve provider schemas and forward the model's tool-call id to Tilde's audited execution.

    ``instructions`` overrides provider descriptions by channel tool name; ``prefix`` namespaces
    model tool names when combining several channel collections. Pass the result as ``tools=``
    to ``Agent``.
    """
    result: list[Tool[Any]] = []
    seen: set[str] = set()
    for name, channel in channels.items():
        key = model_tool_name(f"{prefix}{name}")
        if not key or key in seen:
            raise ValueError("Conflicting or invalid model tool names")
        seen.add(key)
        description = (
            instructions[name] if instructions and name in instructions else channel.description
        )
        result.append(
            Tool.from_schema(
                _execute(channel),
                name=key,
                description=description,
                json_schema=channel.input_schema,
                takes_ctx=True,
            )
        )
    return result


def _execute(channel: ChannelTool | TildeTool):
    # Pydantic AI sets RunContext.tool_call_id to the model's call id before invoking a tool
    # function that takes the context; the schema's fields arrive as keyword arguments.
    async def execute(ctx: RunContext[Any], /, **arguments: Any) -> Any:
        token = _RUN_CONTEXT.set(ctx)
        try:
            return await channel.execute(arguments, tool_call_id=ctx.tool_call_id)
        finally:
            _RUN_CONTEXT.reset(token)

    return execute


async def with_tilde_tools(
    ctx: AgentContext,
    native_tools: Sequence[Tool[Any] | Callable[..., Any]],
    *,
    options: Mapping[str, BundledOptions] | None = None,
) -> AbstractToolset[Any]:
    """One toolset with the current channel's tools, the agent's other Tilde tools and its own
    tools, for ``Agent(toolsets=[...])``.

    ``native_tools`` are ``Tool`` instances or plain functions (``takes_ctx`` inferred). Each is
    published to Tilde with its name, description, parameter and return schemas, so
    ``tools.search`` finds it and a ``tools.execute`` naming it runs it here; every call is
    audited once with the model's call id. Tilde metadata comes from
    ``Tool(..., metadata={"tilde": BundledOptions(...)})`` or ``options[name]``, which wins.
    Call it once per invocation.

    Caveats: tools registered with ``@agent.tool`` are outside this toolset and are neither
    published nor audited; a ``ModelRetry`` is audited as a failed call; a tool's ``prepare``
    function does not change what is published.
    """
    tools = [tool if isinstance(tool, Tool) else Tool(tool) for tool in native_tools]
    channel_tools = convert_to_pydantic_ai_tools({**ctx.channel.current, **ctx.agent_tools})
    toolset = _AuditedToolset(
        FunctionToolset([*channel_tools, *tools]),
        tilde=ctx,
        bundled=frozenset(tool.name for tool in tools),
    )
    definitions = {
        tool.name: bundled_tool(describe_tool(tool, options), _route(toolset, tool.name))
        for tool in tools
    }
    # Published even when empty, so tools removed since the last call stop running here.
    await ctx._set_bundled_tools(definitions)
    return toolset


def describe_tool(tool: Tool[Any], options: Mapping[str, BundledOptions] | None = None) -> ToolSpec:
    """What ``with_tilde_tools`` publishes and ``tilde deploy`` declares for one tool."""
    definition = tool.tool_def
    declared = (definition.metadata or {}).get("tilde")
    return tool_spec(
        tool.name,
        definition.description or "",
        definition.parameters_json_schema,
        output_schema=definition.return_schema,
        options=(options or {}).get(tool.name, declared),
    )


@dataclass
class _AuditedToolset(WrapperToolset[Any]):
    tilde: AgentContext
    bundled: frozenset[str]

    async def call_tool(
        self, name: str, tool_args: dict[str, Any], ctx: RunContext[Any], tool: ToolsetTool[Any]
    ) -> Any:
        if name not in self.bundled:
            return await super().call_tool(name, tool_args, ctx, tool)
        return await self.tilde._run_audited(
            name,
            ctx.tool_call_id or str(uuid.uuid4()),
            tool_args,
            lambda: super(_AuditedToolset, self).call_tool(name, tool_args, ctx, tool),
        )


def _route(toolset: _AuditedToolset, name: str):
    # A tools.execute naming this tool: validate and call it as Pydantic AI would, within the
    # tools.execute call's RunContext, so the audited call_tool above reports it once.
    async def execute(input: Any, call: str) -> Any:
        run = _RUN_CONTEXT.get(None)
        if run is None:
            raise RuntimeError(f"{name} runs through tools.execute only inside a Pydantic AI run")
        step = replace(run, tool_name=name, tool_call_id=call)
        tool = (await toolset.get_tools(step))[name]
        args = tool.args_validator.validate_python(input)
        if tool.args_validator_func is not None:
            checked = tool.args_validator_func(step, **args)
            if inspect.isawaitable(checked):
                await checked
        return await toolset.call_tool(name, args, step, tool)

    return execute
