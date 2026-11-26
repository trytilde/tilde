"""Tilde's per-invocation pieces for a module-level ``create_agent`` graph.

A compiled graph takes no per-call tools or messages: ``ainvoke``'s config carries callbacks
and ``context`` only, and callbacks cannot change state. So the pieces come from middleware
the agent is created with once, ``create_agent(..., middleware=[tilde_middleware(), ...])``,
which reads the invocation running the graph from Tilde's context variable. Outside an
invocation it does nothing.

Tools added to a model request in ``wrap_model_call`` are not registered with the graph's
``ToolNode``; ``create_agent`` allows that only when some middleware executes them in
``wrap_tool_call`` (its "dynamic tools" contract), which this middleware does.

Suspension: before each model call, a Tilde tool call still running after about a minute (it
answered pending) or Tilde's deadline saves the agent state's messages (``messages_to_dict``)
and ends the invocation; the run's next invocation replaces the call's messages with them
(``RemoveMessage(REMOVE_ALL_MESSAGES)``), each pending tool message replaced by the call's
outcome, and the graph continues.
"""

from __future__ import annotations

import json
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
from langchain_core.messages import (
    BaseMessage,
    HumanMessage,
    RemoveMessage,
    SystemMessage,
    ToolMessage,
    messages_from_dict,
    messages_to_dict,
)
from langchain_core.tools import BaseTool
from langgraph.graph.message import REMOVE_ALL_MESSAGES
from langgraph.prebuilt.tool_node import ToolCallRequest
from langgraph.runtime import Runtime
from langgraph.types import Command

from tilde import AgentContext, BundledTools, Resumed, current_invocation
from tilde_langchain.discover import dynamic_stamp, reachable_middleware
from tilde_langchain.tools import convert_to_langchain_tools, with_tilde_tools


@dataclass(slots=True)
class _Invocation:
    tools: dict[str, BaseTool]
    skills: str


# One entry per running invocation; it goes when the invocation's context does.
_invocations: weakref.WeakKeyDictionary[AgentContext, _Invocation] = weakref.WeakKeyDictionary()


@dataclass(slots=True)
class _Loop:
    """The graph's messages at its last step boundary, for suspending it."""

    restored: bool = False
    latest: list[BaseMessage] | None = None


_loops: weakref.WeakKeyDictionary[AgentContext, _Loop] = weakref.WeakKeyDictionary()


def _loop(ctx: AgentContext) -> _Loop:
    found = _loops.get(ctx)
    if found is None:
        found = _loops[ctx] = _Loop()
        ctx.enable_suspension(lambda: messages_to_dict(found.latest or []))
    return found


def _restore(resumed: Resumed) -> list[BaseMessage]:
    """The saved messages, each pending tool message replaced by the call's outcome."""
    messages = messages_from_dict(resumed.state)
    for index, message in enumerate(messages):
        outcome = (
            resumed.outcomes.get(message.tool_call_id) if isinstance(message, ToolMessage) else None
        )
        if outcome is None:
            continue
        failed = outcome.error is not None
        messages[index] = ToolMessage(
            id=message.id,
            name=message.name,
            tool_call_id=message.tool_call_id,
            content=outcome.error if failed else json.dumps(outcome.output),
            status="error" if failed else "success",
        )
    return messages


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
    """Before each model call: check cancellation, continue a resumed run, add steering input as
    user messages (kept in the graph state) and suspend when Tilde asks. Around it: stamp the
    ``@dynamic_prompt`` middleware listed after this one, add the channel's tools, the agent's
    other Tilde tools, its bundled tools (audited and published as ``with_tilde_tools`` does)
    and, when skills are assigned, the skill tools and their summary. Around tool calls: execute
    those tools."""

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
        loop = _loop(ctx)
        # A resumed run continues its saved conversation instead of the call's own messages.
        restored = None
        if ctx.resumed is not None and not loop.restored:
            loop.restored = True
            restored = _restore(ctx.resumed)
        inputs = [HumanMessage(content=input.text) for input in ctx.take_inputs()]
        loop.latest = [*(restored if restored is not None else state["messages"]), *inputs]
        if ctx.should_suspend:
            await ctx.suspend(messages_to_dict(loop.latest))
        if restored is not None:
            return {"messages": [RemoveMessage(id=REMOVE_ALL_MESSAGES), *loop.latest]}
        return {"messages": inputs} if inputs else None

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
