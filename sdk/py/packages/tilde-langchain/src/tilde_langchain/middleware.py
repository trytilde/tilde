"""Tilde's per-invocation pieces for a module-level ``create_agent`` graph.

A compiled graph takes no per-call tools or messages: ``ainvoke``'s config carries callbacks
and ``context`` only, and callbacks cannot change state. So the pieces come from middleware
the agent is created with once, ``create_agent(..., middleware=[tilde_middleware(), ...])``,
which reads the invocation running the graph from Tilde's context variable. Outside an
invocation it does nothing.

Tools added to a model request in ``wrap_model_call`` are not registered with the graph's
``ToolNode``; ``create_agent`` allows that only when some middleware executes them in
``wrap_tool_call`` (its "dynamic tools" contract), which this middleware does.
"""

from __future__ import annotations

import weakref
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

from langchain.agents.middleware import (
    AgentMiddleware,
    AgentState,
    ModelRequest,
    ModelResponse,
)
from langchain_core.messages import HumanMessage, SystemMessage, ToolMessage
from langchain_core.tools import BaseTool
from langgraph.prebuilt.tool_node import ToolCallRequest
from langgraph.runtime import Runtime
from langgraph.types import Command

from tilde import AgentContext, BundledTools, current_invocation
from tilde_langchain.discover import dynamic_stamp, reachable_middleware
from tilde_langchain.tools import convert_to_langchain_tools, with_tilde_tools


@dataclass(slots=True)
class _Invocation:
    tools: dict[str, BaseTool]
    skills: str


# One entry per running invocation; it goes when the invocation's context does.
_invocations: weakref.WeakKeyDictionary[AgentContext, _Invocation] = weakref.WeakKeyDictionary()


async def _invocation(ctx: AgentContext, bundled: BundledTools | None) -> _Invocation:
    found = _invocations.get(ctx)
    if found is None:
        # Plain create_agent has no native skills: tools to read them plus a summary.
        summary = await ctx.skills.summary()
        tools = await with_tilde_tools(
            ctx,
            bundled.tools if bundled else [],
            options=bundled.options if bundled else None,
        )
        if summary:
            tools += convert_to_langchain_tools(ctx.skills.tools())
        found = _invocations[ctx] = _Invocation({tool.name: tool for tool in tools}, summary)
    return found


class TildeMiddleware(AgentMiddleware[AgentState[Any], Any]):
    """Before each model call: check cancellation and add steering input as user messages
    (kept in the graph state). Around it: stamp the ``@dynamic_prompt`` middleware listed
    after this one, add the channel's tools, the agent's other Tilde tools, its bundled tools
    (audited and published as ``with_tilde_tools`` does) and, when skills are assigned, the
    skill tools and their summary. Around tool calls: execute those tools."""

    def __init__(self, bundled: BundledTools | None = None) -> None:
        super().__init__()
        self.bundled = bundled

    async def abefore_model(
        self, state: AgentState[Any], runtime: Runtime[Any]
    ) -> dict[str, Any] | None:
        ctx = current_invocation()
        if ctx is None:
            return None
        ctx.check()
        inputs = ctx.take_inputs()
        if not inputs:
            return None
        return {"messages": [HumanMessage(content=input.text) for input in inputs]}

    async def awrap_model_call(
        self,
        request: ModelRequest[Any],
        handler: Callable[[ModelRequest[Any]], Awaitable[ModelResponse[Any]]],
    ) -> ModelResponse[Any]:
        ctx = current_invocation()
        if ctx is None:
            return await handler(request)
        # `handler` runs the middleware after this one; a dynamic prompt there sets the system
        # prompt this call sends.
        for middleware in reachable_middleware(handler):
            stamp = dynamic_stamp(middleware)
            if stamp is not None:
                ctx.activate_prompt(*stamp)
        invocation = await _invocation(ctx, self.bundled)
        messages = request.messages
        if invocation.skills:
            # A second system message right after the system prompt, which a dynamic prompt
            # after this middleware replaces; this call only, not the graph state.
            messages = [SystemMessage(content=invocation.skills), *messages]
        return await handler(
            request.override(tools=[*request.tools, *invocation.tools.values()], messages=messages)
        )

    async def awrap_tool_call(
        self,
        request: ToolCallRequest,
        handler: Callable[[ToolCallRequest], Awaitable[ToolMessage | Command[Any]]],
    ) -> ToolMessage | Command[Any]:
        ctx = current_invocation()
        invocation = _invocations.get(ctx) if ctx is not None else None
        if request.tool is None and invocation is not None:
            tool = invocation.tools.get(request.tool_call["name"])
            if tool is not None:
                return await handler(request.override(tool=tool))
        return await handler(request)


def tilde_middleware(bundled: BundledTools | None = None) -> TildeMiddleware:
    """Put it first in ``create_agent(middleware=[...])`` so it sees the dynamic prompts.
    ``bundled`` is the agent's ``define_tools(...)``."""
    return TildeMiddleware(bundled)
