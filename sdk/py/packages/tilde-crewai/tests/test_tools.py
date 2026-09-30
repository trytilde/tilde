import asyncio
import json
import threading

import pytest
from crewai import Agent
from crewai.llms.base_llm import BaseLLM
from crewai.tools import tool
from fake_gateway import invocation_context

from tilde import BundledOptions, Tool
from tilde.types.v1 import chat_pb2 as types
from tilde_crewai import convert_to_crewai_tools, with_tilde_tools

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


async def test_conflicting_names_are_rejected():
    channel = FakeChannel()
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_crewai_tools({"a.b": channel, "a_b": channel})
    # CrewAI would present both of these to the model as `send_message`.
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_crewai_tools({"sendMessage": channel, "send_message": channel})


class SequenceLLM(BaseLLM):
    """Makes the scripted tool calls one turn at a time, then finishes."""

    script: list = []

    def supports_function_calling(self) -> bool:
        return True

    def call(self, messages, tools=None, **kwargs):
        if not self.script:
            return "done"
        name, arguments, call = self.script.pop(0)
        function = {"name": name, "arguments": json.dumps(arguments)}
        return [{"id": call, "type": "function", "function": function}]

    async def acall(self, messages, tools=None, **kwargs):
        return self.call(messages, tools)


@tool("Roll dice")
def roll_dice(count: int) -> dict:
    """Roll six-sided dice."""
    return {"rolls": [4] * count}


@tool("Broken")
def broken() -> str:
    """Always fails."""
    raise RuntimeError("dice fell off the table")


async def test_native_tools_are_published_audited_once_and_routed_through_tools_execute():
    async with invocation_context() as (ctx, gateway):
        tools = await with_tilde_tools(
            ctx,
            [roll_dice, broken],
            options={"Roll dice": BundledOptions(summary="Rolled dice", display="summary")},
        )
        assert "roll_dice" not in ctx.agent_tools
        llm = SequenceLLM(
            model="scripted",
            script=[
                ("roll_dice", {"count": 2}, "call-1"),
                # Found by tools.search: Tilde's tools.execute runs it here.
                ("tools_execute", {"name": "roll_dice", "input": {"count": 1}}, "call-2"),
                ("broken", {}, "call-3"),
            ],
        )
        agent = Agent(role="Assistant", goal="Roll", backstory="Test", llm=llm, tools=tools)
        await agent.kickoff_async([{"role": "user", "content": "Roll"}])

    _, (dice, failing) = gateway.bundled_registrations
    # Published under the name CrewAI shows the model.
    assert (dice.name, dice.description) == ("roll_dice", "Roll six-sided dice.")
    assert (dice.summary, dice.display) == ("Rolled dice", types.TOOL_DISPLAY_SUMMARY)
    assert json.loads(dice.input_schema_json)["properties"]["count"]["type"] == "integer"
    assert failing.name == "broken"
    assert gateway.tool_calls == []
    reports = gateway.tool_reports
    assert [(c.name, c.status) for c in reports] == [
        ("roll_dice", "running"),
        ("roll_dice", "completed"),
        ("roll_dice", "running"),
        ("roll_dice", "completed"),
        ("broken", "running"),
        ("broken", "failed"),
    ]
    # CrewAI has no model call id: each call is audited under its own generated id.
    assert [reports[i].id == reports[i + 1].id for i in (0, 2, 4)] == [True] * 3
    assert len({reports[i].id for i in (0, 2, 4)}) == 3
    assert json.loads(reports[1].output_json) == {"rolls": [4, 4]}
    assert reports[5].error == "dice fell off the table"


async def test_with_no_tools_unpublishes_the_last_bundled_tool():
    async with invocation_context() as (ctx, gateway):
        await with_tilde_tools(ctx, [roll_dice])
        (old,) = gateway.bundled_registrations[-1]
        tools = await with_tilde_tools(ctx, [])
        assert old.name not in {t.name for t in tools}
        # The empty set is registered, so tools.execute naming the old tool goes to Tilde.
        assert gateway.bundled_registrations[-1] == []
        output = await ctx.tools["tools.execute"].execute(
            {"name": old.name, "input": {"count": 1}}, tool_call_id="call-1"
        )
    assert output == {"ok": True, "input": {"name": old.name, "input": {"count": 1}}}
    assert [frames[0].name for frames in gateway.tool_calls] == ["tools.execute"]
    assert gateway.tool_reports == []


# The server allows a 64-character tool name after its source slug, so catalog names can be
# longer than the 64 characters models accept.
STAGES = "customer_relationship_hub.list_open_opportunities_for_owner_by_stages"
REGION = "customer_relationship_hub.list_open_opportunities_for_owner_by_region"


async def test_long_catalog_names_get_stable_distinct_aliases_bound_to_their_tools():
    calls: list[str] = []

    def catalog_tool(name: str) -> Tool:
        async def execute(input, call):
            calls.append(name)
            return {"ok": True}

        return Tool(description=name, input_schema={"type": "object"}, _execute=execute)

    tools = convert_to_crewai_tools({STAGES: catalog_tool(STAGES), REGION: catalog_tool(REGION)})
    stages, region = (t.name for t in tools)
    # The same alias in every adapter and in the TypeScript SDK.
    assert stages == "customer_relationship_hub_list_open_opportunities_for_o_d9926af9"
    assert len(region) == 64 and region != stages
    await tools[1]._arun()
    assert calls == [REGION]
