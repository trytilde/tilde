"""Expose Tilde channel tools to CrewAI with provider descriptions and schemas intact, and
publish the agent's own CrewAI tools to Tilde as bundled tools."""

from __future__ import annotations

import asyncio
import copy
import json
import uuid
from collections.abc import Mapping, Sequence
from typing import Any

from crewai.tools import BaseTool
from crewai.utilities.string_utils import sanitize_tool_name
from pydantic import BaseModel, PrivateAttr

from tilde import AgentContext, BundledOptions, ChannelTool, Tool
from tilde._tools import ToolSpec, bundled_tool, model_tool_name, tool_spec


class ChannelCrewTool(BaseTool):
    """A channel tool executed on the agent's event loop from wherever CrewAI calls it."""

    _channel: ChannelTool | Tool = PrivateAttr()
    _loop: asyncio.AbstractEventLoop = PrivateAttr()

    def _run(self, **arguments: Any) -> str:
        # CrewAI's executor calls tools synchronously on a worker thread, while channel
        # execution belongs to the invocation's event loop.
        try:
            running = asyncio.get_running_loop()
        except RuntimeError:
            running = None
        if running is self._loop:
            raise RuntimeError("Channel tools block when run on the event loop; await arun()")
        future = asyncio.run_coroutine_threadsafe(self._arun(**arguments), self._loop)
        return future.result()

    async def _arun(self, **arguments: Any) -> str:
        # CrewAI does not pass the model's tool-call id to tools; the core generates one.
        result = await self._channel.execute(arguments)
        # CrewAI renders results with str(); JSON reads better to the model than a Python repr.
        return result if isinstance(result, str) else json.dumps(result, default=str)


def convert_to_crewai_tools(
    channels: Mapping[str, ChannelTool | Tool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[BaseTool]:
    """Build CrewAI tools that keep provider descriptions and JSON schemas.

    Call this on the invocation's event loop: the loop is captured so that CrewAI's worker
    threads can execute channel tools on it. CrewAI reads a tool's parameters from
    ``args_schema.model_json_schema()``, so the schema model returns the provider schema rather
    than one introspected from fields; having no fields, it also passes arguments through
    unvalidated. CrewAI then applies its own strict-mode pass and name normalization (see the
    README). ``instructions`` overrides descriptions by channel name; ``prefix`` namespaces
    model tool names when combining collections.
    """
    loop = asyncio.get_running_loop()
    result: list[BaseTool] = []
    names: set[str] = set()
    for name, channel in channels.items():
        key = model_tool_name(f"{prefix}{name}")
        # CrewAI lowercases and snake_cases names for the model and would silently suffix a
        # duplicate, so conflicts are checked on the name the model sees.
        model_name = sanitize_tool_name(key)
        if not key or not model_name or model_name in names:
            raise ValueError("Conflicting or invalid model tool names")
        names.add(model_name)
        description = (
            instructions[name]
            if instructions is not None and name in instructions
            else channel.description
        )
        tool = ChannelCrewTool(
            name=key, description=description, args_schema=_schema_model(channel.input_schema)
        )
        tool._channel = channel
        tool._loop = loop
        result.append(tool)
    return result


def _schema_model(schema: dict[str, Any]) -> type[BaseModel]:
    class ChannelToolInput(BaseModel):
        @classmethod
        def model_json_schema(cls, *args: Any, **kwargs: Any) -> dict[str, Any]:
            return copy.deepcopy(schema)

    return ChannelToolInput


class _AuditedCrewTool(BaseTool):
    """One of the agent's own CrewAI tools, audited on the invocation's event loop."""

    _tool: BaseTool = PrivateAttr()
    _ctx: AgentContext = PrivateAttr()
    _loop: asyncio.AbstractEventLoop = PrivateAttr()
    _published: str = PrivateAttr()

    def _run(self, **arguments: Any) -> Any:
        # CrewAI calls tools synchronously on a worker thread; audits belong to the loop.
        try:
            running = asyncio.get_running_loop()
        except RuntimeError:
            running = None
        if running is self._loop:
            raise RuntimeError("Audited tools block when run on the event loop; await arun()")
        return asyncio.run_coroutine_threadsafe(self._arun(**arguments), self._loop).result()

    async def _arun(self, **arguments: Any) -> Any:
        # CrewAI never passes the model's tool-call id to tools; audit under a fresh one.
        return await self._call(arguments, str(uuid.uuid4()))

    async def _call(self, arguments: dict[str, Any], call: str) -> Any:
        return await self._ctx._run_audited(
            self._published, call, arguments, lambda: asyncio.to_thread(self._invoke, arguments)
        )

    def _invoke(self, arguments: dict[str, Any]) -> Any:
        # Arguments were validated by this tool's run(); run the wrapped tool's own body.
        result = self._tool._run(**arguments)
        return asyncio.run(result) if asyncio.iscoroutine(result) else result


async def with_tilde_tools(
    ctx: AgentContext,
    native_tools: Sequence[BaseTool],
    *,
    options: Mapping[str, BundledOptions] | None = None,
) -> list[BaseTool]:
    """The current channel's tools, the agent's other Tilde tools and its own tools, for
    ``Agent(tools=...)``. Call it on the invocation's event loop.

    Each native tool (``@tool`` or a ``BaseTool`` subclass) is published to Tilde under the name
    CrewAI shows the model, with its description and argument and result schemas, so
    ``tools.search`` finds it and a ``tools.execute`` naming it runs it here. It is returned as
    a ``BaseTool`` that delegates to it, keeping its schemas, ``result_as_answer``, usage limit,
    cache function and failure policy, and audits every call once. CrewAI has no
    free metadata, so the summary and display come from ``options[name]`` (the tool's own name).

    Caveats: CrewAI never passes the model's tool-call id to tools, so direct calls are audited
    under a generated id that does not match the model's; ``tools.execute`` calls use their own
    call id. A ``tools.execute`` call does not count towards the tool's usage limit.
    """
    loop = asyncio.get_running_loop()
    tools = convert_to_crewai_tools({**ctx.channel.current, **ctx.agent_tools})
    names = {sanitize_tool_name(tool.name) for tool in tools}
    definitions = {}
    for native in native_tools:
        published = sanitize_tool_name(native.name)
        if not published or published in names:
            raise ValueError(f"Tool {native.name} conflicts with another tool")
        names.add(published)
        tool = _AuditedCrewTool(
            name=native.name,
            description=native.description,
            env_vars=native.env_vars,
            args_schema=native.args_schema,
            result_schema=native.result_schema,
            cache_function=native.cache_function,
            result_as_answer=native.result_as_answer,
            max_usage_count=native.max_usage_count,
            tool_failure_policy=native.tool_failure_policy,
        )
        tool._tool = native
        tool._ctx = ctx
        tool._loop = loop
        tool._published = published
        definitions[published] = bundled_tool(describe_tool(native, options), _route(tool))
        tools.append(tool)
    # Published even when empty, so tools removed since the last call stop running here.
    await ctx._set_bundled_tools(definitions)
    return tools


def describe_tool(
    native: BaseTool, options: Mapping[str, BundledOptions] | None = None
) -> ToolSpec:
    """What ``with_tilde_tools`` publishes and ``tilde deploy`` declares for one tool: named as
    CrewAI shows it to the model, with options looked up by the tool's own name."""
    schema = native.args_schema.model_json_schema()
    schema.pop("title", None)
    return tool_spec(
        sanitize_tool_name(native.name),
        native.description,
        schema,
        output_schema=native.result_schema.model_json_schema() if native.result_schema else None,
        options=(options or {}).get(native.name),
    )


def _route(tool: _AuditedCrewTool):
    # A tools.execute naming this tool: validate as CrewAI would, audit under that call's id.
    async def execute(input: Any, call: str) -> Any:
        return await tool._call(tool._validate_kwargs(input), call)

    return execute
