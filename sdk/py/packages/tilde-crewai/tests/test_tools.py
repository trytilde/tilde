import asyncio
import threading

import pytest
from crewai import Agent
from crewai.llms.base_llm import BaseLLM

from tilde_crewai import convert_to_crewai_tools

SCHEMA = {
    "type": "object",
    "properties": {
        "value": {"type": "number"},
        "target": {
            "type": "object",
            "properties": {"thread": {"type": "string"}, "note": {"type": "string"}},
            "required": ["thread"],
        },
    },
    "required": ["value"],
}


class FakeChannel:
    def __init__(self) -> None:
        self.description = "Provider instructions"
        self.input_schema = SCHEMA
        self.calls: list[tuple[dict, str | None]] = []
        self.threads: list[threading.Thread] = []

    async def execute(self, input, *, tool_call_id=None):
        self.calls.append((input, tool_call_id))
        self.threads.append(threading.current_thread())
        return {"ok": True}


class ScriptedLLM(BaseLLM):
    """Requests one tool call, then finishes; records what CrewAI sent to the provider."""

    requests: list = []

    def supports_function_calling(self) -> bool:
        return True

    def call(self, messages, tools=None, **kwargs):
        self.requests.append({"messages": [dict(m) for m in messages], "tools": tools})
        if len(self.requests) > 1:
            return "done"
        arguments = '{"value": 3, "target": {"thread": "t"}}'
        function = {"name": tools[0]["function"]["name"], "arguments": arguments}
        return [{"id": "call-123", "type": "function", "function": function}]

    async def acall(self, messages, tools=None, **kwargs):
        return self.call(messages, tools)


async def test_tool_conversion_preserves_schema_and_overrides_instructions():
    channel = FakeChannel()
    [tool] = convert_to_crewai_tools(
        {"customerAction": channel},
        prefix="custom_",
        instructions={"customerAction": "Custom model instructions"},
    )
    assert tool.name == "custom_customerAction"
    assert tool.description == "Custom model instructions"
    assert tool.args_schema.model_json_schema() == SCHEMA
    [default] = convert_to_crewai_tools({"customerAction": channel})
    assert default.description == "Provider instructions"


async def test_tools_execute_on_the_event_loop_from_the_loop_and_from_a_worker_thread():
    channel = FakeChannel()
    [tool] = convert_to_crewai_tools({"action": channel})
    assert await tool.arun(value=1) == '{"ok": true}'
    assert await asyncio.to_thread(tool.run, value=2, target={"thread": "t"}) == '{"ok": true}'
    # Arguments are not rewritten by an introspected model, and CrewAI has no tool-call id.
    assert channel.calls == [({"value": 1}, None), ({"value": 2, "target": {"thread": "t"}}, None)]
    assert channel.threads == [threading.main_thread()] * 2
    with pytest.raises(RuntimeError, match="await arun"):
        tool.run(value=3)


async def test_crewai_agent_sends_provider_schema_and_history_and_executes_the_channel_tool():
    channel = FakeChannel()
    llm = ScriptedLLM(model="scripted")
    agent = Agent(
        role="Assistant",
        goal="Respond",
        backstory="Test",
        llm=llm,
        tools=convert_to_crewai_tools({"sendMessage": channel}),
        max_iter=3,
    )
    image = {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
    await agent.kickoff_async(
        [
            {"role": "user", "content": "Hi"},
            {"role": "assistant", "content": "Hello"},
            {"role": "user", "content": [{"type": "text", "text": "Look"}, image]},
            {"role": "user", "content": "Respond"},
        ]
    )
    first, second = llm.requests
    # CrewAI's model-facing name is lowercased snake_case, and its OpenAI strict pass marks
    # every property required and closes objects; types, nesting and descriptions survive.
    assert first["tools"] == [
        {
            "type": "function",
            "function": {
                "name": "send_message",
                "description": "Provider instructions",
                "strict": True,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "value": {"type": "number"},
                        "target": {
                            "type": "object",
                            "properties": {
                                "thread": {"type": "string"},
                                "note": {"type": "string"},
                            },
                            "required": ["thread", "note"],
                            "additionalProperties": False,
                        },
                    },
                    "required": ["value", "target"],
                    "additionalProperties": False,
                },
            },
        }
    ]
    history = [(m["role"], m["content"]) for m in first["messages"][1:]]
    assert history[:3] == [
        ("user", "Hi"),
        ("assistant", "Hello"),
        ("user", [{"type": "text", "text": "Look"}, image]),
    ]
    assert history[3][0] == "user" and history[3][1].endswith("Respond")
    assert channel.calls == [({"value": 3, "target": {"thread": "t"}}, None)]
    assert channel.threads == [threading.main_thread()]
    assert second["messages"][-1]["role"] == "tool"
    assert second["messages"][-1]["tool_call_id"] == "call-123"
    assert second["messages"][-1]["content"] == '{"ok": true}'


async def test_conflicting_or_invalid_names_are_rejected():
    channel = FakeChannel()
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_crewai_tools({"a.b": channel, "a_b": channel})
    # CrewAI would present both of these to the model as `send_message`.
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_crewai_tools({"sendMessage": channel, "send_message": channel})
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_crewai_tools({"x" * 65: channel})
