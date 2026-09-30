"""Expose Tilde channel tools as LangChain tools with the model's tool-call id forwarded, and
publish the agent's own LangChain tools to Tilde as bundled tools."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any
from uuid import UUID

from langchain_core.callbacks import AsyncCallbackHandler, BaseCallbackManager
from langchain_core.messages import ToolMessage
from langchain_core.tools import BaseTool
from langchain_core.utils.function_calling import convert_to_openai_tool

from tilde import AgentContext, BundledOptions, ChannelTool, Tool
from tilde._tools import ToolSpec, bundled_tool, model_tool_name, tool_spec


class ChannelToolAdapter(BaseTool):
    """Async-only LangChain tool over one Tilde channel tool.

    LangChain passes the model's tool-call id into ``BaseTool.arun`` whenever the tool is
    invoked with a ``ToolCall`` dict (which is what LangGraph's ``ToolNode`` and therefore
    ``create_agent`` do). With a JSON-schema ``args_schema`` nothing injects that id into the
    coroutine arguments, so ``_to_args_and_kwargs`` is overridden to carry it into ``_arun``.
    When invoked directly with plain arguments there is no call id; ``ChannelTool.execute``
    then generates a random UUID, so the call is audited but not deduplicated.
    """

    channel: ChannelTool | Tool

    model_config = {"arbitrary_types_allowed": True}

    def _to_args_and_kwargs(
        self, tool_input: str | dict[str, Any], tool_call_id: str | None
    ) -> tuple[tuple[str, ...], dict[str, Any]]:
        return (), {"input": tool_input, "tool_call_id": tool_call_id}

    def _run(self, *args: Any, **kwargs: Any) -> Any:
        raise NotImplementedError("Channel tools run on the agent's event loop; use ainvoke")

    async def _arun(self, input: Any, tool_call_id: str | None = None) -> Any:
        return await self.channel.execute(input, tool_call_id=tool_call_id)


def convert_to_langchain_tools(
    channels: Mapping[str, ChannelTool | Tool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[BaseTool]:
    """Preserve provider schemas and descriptions; ``instructions`` overrides descriptions
    without touching schemas or authorization, ``prefix`` namespaces model tool names.
    ``ctx.skills.tools()`` converts the same way."""
    result: dict[str, BaseTool] = {}
    for name, channel in channels.items():
        key = model_tool_name(f"{prefix}{name}")
        if not key or key in result:
            raise ValueError("Conflicting or invalid model tool names")
        description = (
            instructions[name]
            if instructions is not None and name in instructions
            else channel.description
        )
        result[key] = ChannelToolAdapter(
            name=key,
            description=description,
            args_schema=channel.input_schema,
            channel=channel,
        )
    return list(result.values())


async def with_tilde_tools(
    ctx: AgentContext,
    native_tools: Sequence[BaseTool],
    *,
    options: Mapping[str, BundledOptions] | None = None,
) -> list[BaseTool]:
    """The current channel's tools, the agent's other Tilde tools and its own tools, for
    ``create_agent(tools=...)`` or a ``ToolNode``.

    Each native tool (``@tool``, ``StructuredTool`` or any ``BaseTool``) is published to Tilde
    with its name, description and the argument schema the model sees, so ``tools.search``
    finds it and a ``tools.execute`` naming it runs it here. It is returned as a copy of the
    same type with an audit callback handler attached, so ``ToolNode`` injection keeps working
    and every call is audited once with the model's call id. Tilde metadata comes from
    ``metadata={"tilde": BundledOptions(...)}`` or ``options[name]``, which wins. Call it once per
    invocation.

    Caveats: LangChain has no output schema, and audits record the tool message content. A
    tool that needs ``ToolRuntime`` or ``InjectedState`` only gets them from ``ToolNode``, so it
    cannot run through ``tools.execute``. Invoked without a ``ToolCall`` (no model call id) the
    call is audited under LangChain's run id.
    """
    tools = convert_to_langchain_tools({**ctx.channel.current, **ctx.agent_tools})
    names = {tool.name for tool in tools}
    definitions = {}
    for native in native_tools:
        if native.name in names:
            raise ValueError(f"Tool {native.name} conflicts with another tool")
        names.add(native.name)
        tool = _audited(ctx, native)
        definitions[tool.name] = bundled_tool(describe_tool(native, options), _route(tool))
        tools.append(tool)
    # Published even when empty, so tools removed since the last call stop running here.
    await ctx._set_bundled_tools(definitions)
    return tools


def describe_tool(tool: BaseTool, options: Mapping[str, BundledOptions] | None = None) -> ToolSpec:
    """What ``with_tilde_tools`` publishes and ``tilde deploy`` declares for one tool."""
    return tool_spec(
        tool.name,
        tool.description,
        convert_to_openai_tool(tool)["function"]["parameters"],
        options=(options or {}).get(tool.name, (tool.metadata or {}).get("tilde")),
    )


class _Audit(AsyncCallbackHandler):
    """Reports one tool's calls. Attached to that tool only, it is not inherited by child runs;
    ``raise_error`` surfaces failed reports instead of LangChain logging them."""

    raise_error = True

    def __init__(self, ctx: AgentContext, name: str) -> None:
        self._ctx = ctx
        self._name = name
        self._calls: dict[UUID, str] = {}

    async def on_tool_start(
        self,
        serialized: dict[str, Any],
        input_str: str,
        *,
        run_id: UUID,
        inputs: dict[str, Any] | None = None,
        tool_call_id: str | None = None,
        **kwargs: Any,
    ) -> None:
        call = self._calls[run_id] = tool_call_id or str(run_id)
        input = inputs if inputs is not None else input_str
        await self._ctx._report_bundled(self._name, call, "running", input=input)

    async def on_tool_end(self, output: Any, *, run_id: UUID, **kwargs: Any) -> None:
        call = self._calls.pop(run_id, str(run_id))
        # A ToolMessage with error status is a failure LangChain handled for the model.
        if isinstance(output, ToolMessage) and output.status == "error":
            await self._ctx._report_bundled(self._name, call, "failed", error=str(output.content))
            return
        content = output.content if isinstance(output, ToolMessage) else output
        await self._ctx._report_bundled(self._name, call, "completed", output=content)

    async def on_tool_error(self, error: BaseException, *, run_id: UUID, **kwargs: Any) -> None:
        call = self._calls.pop(run_id, str(run_id))
        await self._ctx._report_bundled(self._name, call, "failed", error=str(error))


def _audited(ctx: AgentContext, tool: BaseTool) -> BaseTool:
    # Not a proxy: a copy of the tool itself keeps its type, schema and injected arguments.
    handler = _Audit(ctx, tool.name)
    callbacks = tool.callbacks
    if isinstance(callbacks, BaseCallbackManager):
        callbacks = callbacks.copy()
        callbacks.add_handler(handler, inherit=False)
    else:
        callbacks = [*(callbacks or []), handler]
    return tool.model_copy(update={"callbacks": callbacks})


def _route(tool: BaseTool):
    # A tools.execute naming this tool invokes it with a ToolCall carrying the tools.execute
    # call id; it runs inside that call's LangChain config, so traces nest.
    async def execute(input: Any, call: str) -> Any:
        result = await tool.ainvoke(
            {"type": "tool_call", "id": call, "name": tool.name, "args": input}
        )
        return result.content if isinstance(result, ToolMessage) else result

    return execute
