"""A create_agent graph with `tilde_middleware()` in a real invocation context: a Tilde tool that
answers pending suspends the graph before the next model call with its messages saved; the run's
next invocation continues from them with the call's outcome in place of the pending answer."""

from __future__ import annotations

import json
from typing import Any

import pytest
from fake_gateway import DEPLOY, FakeGateway, invocation_context
from langchain.agents import create_agent
from langchain_core.language_models import BaseChatModel
from langchain_core.messages import AIMessage, BaseMessage, HumanMessage, ToolMessage
from langchain_core.outputs import ChatGeneration, ChatResult

from tilde import StopLoop
from tilde._invocation import current
from tilde_langchain import tilde_middleware


class Scripted(BaseChatModel):
    """Answers with the next scripted message and records what it was sent."""

    replies: list[AIMessage]
    seen: list[list[BaseMessage]] = []

    @property
    def _llm_type(self) -> str:
        return "scripted"

    def bind_tools(self, tools: Any, **kwargs: Any) -> Scripted:
        return self

    def _generate(self, messages, stop=None, run_manager=None, **kwargs) -> ChatResult:
        self.seen.append(list(messages))
        return ChatResult(generations=[ChatGeneration(message=self.replies.pop(0))])


DEPLOY_CALL = AIMessage(content="", tool_calls=[{"id": "c1", "name": "ops_deploy", "args": {}}])


async def test_a_pending_tool_call_suspends_the_graph_and_the_next_invocation_continues() -> None:
    gateway = FakeGateway(tools=[DEPLOY], pending_tools={"ops.deploy"})
    async with invocation_context(gateway) as (ctx, _):
        current.set(ctx)
        model = Scripted(replies=[DEPLOY_CALL], seen=[])
        agent = create_agent(model=model, middleware=[tilde_middleware()])
        with pytest.raises(StopLoop):
            await agent.ainvoke({"messages": [HumanMessage("Deploy it")]})
        assert len(model.seen) == 1, "no model call after the pending answer"
    (saved,) = gateway.suspensions
    (call,) = saved.pending_call_ids
    state = json.loads(saved.state_json)
    assert [m["type"] for m in state["state"]] == ["human", "ai", "tool"]
    assert state["pending"][0]["toolCallId"] == "c1"

    gateway.outcomes[call] = json.dumps({"version": 7})
    async with invocation_context(gateway, resumes=True) as (ctx, _):
        current.set(ctx)
        model = Scripted(replies=[AIMessage("Deployed")], seen=[])
        agent = create_agent(model=model, middleware=[tilde_middleware()])
        result = await agent.ainvoke({"messages": [HumanMessage("(woken)")]})
    seen = model.seen[0]
    assert [type(m).__name__ for m in seen] == ["HumanMessage", "AIMessage", "ToolMessage"]
    assert seen[0].content == "Deploy it", "the saved conversation, not the call's own"
    assert isinstance(seen[2], ToolMessage) and json.loads(seen[2].content) == {"version": 7}
    assert result["messages"][-1].content == "Deployed"
    assert len(gateway.suspensions) == 1, "nothing pending, nothing saved"
