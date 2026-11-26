"""A Pydantic AI run with `tilde_pydantic_ai` in a real invocation context: a Tilde tool that
answers pending suspends the run before the next model request with its history saved; the
run's next invocation continues from it with the call's outcome in place of the pending answer."""

from __future__ import annotations

import json

import pytest
from fake_gateway import DEPLOY, FakeGateway, invocation_context
from pydantic_ai import Agent
from pydantic_ai.exceptions import RunCancelled
from pydantic_ai.messages import ModelMessage, ModelResponse, TextPart, ToolCallPart, ToolReturnPart
from pydantic_ai.models.function import AgentInfo, FunctionModel

from tilde import StopLoop
from tilde_pydantic_ai import tilde_pydantic_ai


async def test_a_pending_tool_call_suspends_the_run_and_the_next_invocation_continues() -> None:
    requests: list[list[ModelMessage]] = []

    def deploy(messages: list[ModelMessage], _info: AgentInfo) -> ModelResponse:
        requests.append(list(messages))
        return ModelResponse(parts=[ToolCallPart("ops_deploy", {}, tool_call_id="c1")])

    gateway = FakeGateway(tools=[DEPLOY], pending_tools={"ops.deploy"})
    async with invocation_context(gateway) as (ctx, _):
        # Saving cancels the run through its cancellation token.
        with pytest.raises((StopLoop, RunCancelled)):
            await Agent(FunctionModel(deploy)).run(
                "Deploy it", **await tilde_pydantic_ai(ctx, skills=False)
            )
        assert ctx.cancelled
    assert len(requests) == 1, "no model request after the pending answer"
    (saved,) = gateway.suspensions
    (call,) = saved.pending_call_ids
    state = json.loads(saved.state_json)["state"]
    assert [m["kind"] for m in state] == ["request", "response", "request"]

    seen: list[list[ModelMessage]] = []

    def finish(messages: list[ModelMessage], _info: AgentInfo) -> ModelResponse:
        seen.append(list(messages))
        return ModelResponse(parts=[TextPart("Deployed")])

    gateway.outcomes[call] = json.dumps({"version": 7})
    async with invocation_context(gateway, resumes=True) as (ctx, _):
        result = await Agent(FunctionModel(finish)).run(
            "(woken)", **await tilde_pydantic_ai(ctx, skills=False)
        )
    history = seen[0]
    assert history[0].parts[0].content == "Deploy it", "the saved history, not the call's own"
    returned = [p for p in history[-1].parts if isinstance(p, ToolReturnPart)]
    assert returned[0].tool_call_id == "c1" and returned[0].content == {"version": 7}
    assert result.output == "Deployed"
