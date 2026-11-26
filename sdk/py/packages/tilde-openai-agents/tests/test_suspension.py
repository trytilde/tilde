"""An OpenAI Agents run with `tilde_openai_agents` in a real invocation context: a Tilde tool that
answers pending suspends the run before the next model call with its items saved; the run's next
invocation continues from them with the call's outcome in place of the pending answer."""

from __future__ import annotations

import json
from typing import Any

import pytest
from agents import Agent, Runner, Usage, set_tracing_disabled
from agents.items import ModelResponse
from agents.models.interface import Model
from fake_gateway import DEPLOY, FakeGateway, invocation_context
from openai.types.responses import (
    ResponseFunctionToolCall,
    ResponseOutputMessage,
    ResponseOutputText,
)

from tilde import StopLoop
from tilde_openai_agents import tilde_openai_agents

set_tracing_disabled(True)


class Scripted(Model):
    """Answers with the next scripted output and records each call's input."""

    def __init__(self, *turns: list[Any]) -> None:
        self.turns = list(turns)
        self.inputs: list[Any] = []

    async def get_response(self, system_instructions, input, *args: Any, **kwargs: Any):
        self.inputs.append(json.loads(json.dumps(input, default=lambda i: i.model_dump())))
        return ModelResponse(output=self.turns.pop(0), usage=Usage(), response_id=None)

    def stream_response(self, *args: Any, **kwargs: Any):
        raise NotImplementedError


DEPLOY_CALL = [
    ResponseFunctionToolCall(
        type="function_call", id="fc1", call_id="c1", name="ops_deploy", arguments="{}"
    )
]
DONE = [
    ResponseOutputMessage(
        id="m1",
        type="message",
        role="assistant",
        status="completed",
        content=[ResponseOutputText(type="output_text", text="Deployed", annotations=[])],
    )
]


async def run(ctx, model: Scripted, input: Any):
    agent, config = await tilde_openai_agents(ctx, Agent(name="shipper", model=model))
    return await Runner.run(agent, input, run_config=config, max_turns=5)


async def test_a_pending_tool_call_suspends_the_run_and_the_next_invocation_continues() -> None:
    gateway = FakeGateway(tools=[DEPLOY], pending_tools={"ops.deploy"})
    async with invocation_context(gateway) as (ctx, _):
        model = Scripted(DEPLOY_CALL, DONE)
        with pytest.raises(StopLoop):
            await run(ctx, model, "Deploy it")
        assert len(model.inputs) == 1, "no model call after the pending answer"
    (saved,) = gateway.suspensions
    (call,) = saved.pending_call_ids
    state = json.loads(saved.state_json)["state"]
    assert [item.get("type", item.get("role")) for item in state] == [
        "user",
        "function_call",
        "function_call_output",
    ]

    gateway.outcomes[call] = json.dumps({"version": 7})
    async with invocation_context(gateway, resumes=True) as (ctx, _):
        model = Scripted(DONE)
        result = await run(ctx, model, "(woken)")
    sent = model.inputs[0]
    assert sent[0]["content"] == "Deploy it", "the saved items, not the run's own input"
    assert sent[-1]["type"] == "function_call_output"
    assert json.loads(sent[-1]["output"]) == {"version": 7}
    assert result.final_output == "Deployed"
