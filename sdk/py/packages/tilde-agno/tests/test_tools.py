import json
from types import SimpleNamespace

import pytest
from agno.tools import tool
from agno.tools.function import FunctionCall
from fake_gateway import invocation_context

from tilde import BundledOptions, Tool, tool_id
from tilde.types.v1 import chat_pb2 as types
from tilde_agno import convert_to_agno_tools, with_tilde_tools

SCHEMA = {"type": "object", "properties": {"value": {"type": "number"}}, "required": ["value"]}


class FakeChannel:
    def __init__(self) -> None:
        self.description = "Provider instructions"
        self.input_schema = SCHEMA
        self.calls: list[tuple[dict, str | None]] = []

    async def execute(self, input, *, tool_call_id=None):
        self.calls.append((input, tool_call_id))
        return {"ok": True}


async def test_tool_conversion_preserves_schema_overrides_instructions_and_forwards_call_id():
    channel = FakeChannel()
    [function] = convert_to_agno_tools(
        {"customer_action": channel},
        prefix="custom_",
        instructions={"customer_action": "Custom model instructions"},
    )
    function.process_entrypoint()  # What Agent does when registering tools.
    assert function.to_dict() == {
        "name": "custom_customer_action",
        "description": "Custom model instructions",
        "parameters": SCHEMA,
    }
    # Agno's own execution path: the model's tool call id arrives as FunctionCall.call_id.
    call = FunctionCall(function=function, arguments={"value": 3}, call_id="call-123")
    execution = await call.aexecute()
    assert execution.status == "success" and call.result == {"ok": True}
    assert channel.calls == [({"value": 3}, "call-123")]


def test_conflicting_names_are_rejected():
    channel = FakeChannel()
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_agno_tools({"a.b": channel, "a_b": channel})


def test_schema_property_named_fc_is_rejected():
    from tilde_agno.tools import convert_to_agno_tools

    class Channel:
        description = "x"
        input_schema = {"type": "object", "properties": {"fc": {"type": "string"}}}

    with pytest.raises(ValueError, match="fc"):
        convert_to_agno_tools({"tool": Channel()})


def roll_dice(count: int = 1) -> dict:
    """Roll six-sided dice."""
    return {"rolls": [4] * count}


def agent_name(agent) -> str:
    """The name of the agent running this tool."""
    return agent.name


async def test_native_tools_are_published_audited_once_and_routed_through_tools_execute():
    finished: list[str] = []

    @tool(post_hook=lambda fc: finished.append(fc.function.name))
    def broken() -> str:
        """Always fails."""
        raise RuntimeError("dice fell off the table")

    async with invocation_context() as (ctx, gateway):
        functions = await with_tilde_tools(
            ctx,
            [roll_dice, agent_name, broken],
            options={
                "roll_dice": BundledOptions(summary="Rolled dice", display="summary"),
            },
        )
        by_name = {function.name: function for function in functions}
        assert "roll_dice" not in ctx.agent_tools
        for function in functions:
            function.process_entrypoint()  # What Agent does when registering tools.

        # Agno's own execution path: the model's tool call id arrives as FunctionCall.call_id.
        dice = FunctionCall(function=by_name["roll_dice"], arguments={"count": 2}, call_id="call-1")
        assert (await dice.aexecute()).status == "success" and dice.result == {"rolls": [4, 4]}

        # Found by tools.search: Tilde's tools.execute runs it here, as the same agent.
        execute = by_name["tools_execute"]
        execute._agent = SimpleNamespace(name="Bot")
        routed = FunctionCall(
            function=execute, arguments={"name": "agent_name", "input": {}}, call_id="call-2"
        )
        assert (await routed.aexecute()).status == "success" and routed.result == "Bot"

        failing = FunctionCall(function=by_name["broken"], arguments={}, call_id="call-3")
        assert (await failing.aexecute()).status == "failure"
        assert finished == ["broken"]  # The tool's own post hook still runs.

    _, (dice_definition, name_definition, broken_definition) = gateway.bundled_registrations
    assert dice_definition.description == "Roll six-sided dice."
    assert (dice_definition.summary, dice_definition.display) == (
        "Rolled dice",
        types.TOOL_DISPLAY_SUMMARY,
    )
    assert json.loads(dice_definition.input_schema_json)["properties"]["count"]["type"] == "integer"
    # Injected framework arguments are not part of the published schema.
    assert "agent" not in json.loads(name_definition.input_schema_json)["properties"]
    assert broken_definition.name == "broken"
    assert gateway.tool_calls == []
    assert [(c.id, c.name, c.status) for c in gateway.tool_reports] == [
        (tool_id(ctx.invocation_id, call, name), name, status)
        for call, name, status in [
            ("call-1", "roll_dice", "running"),
            ("call-1", "roll_dice", "completed"),
            ("call-2", "agent_name", "running"),
            ("call-2", "agent_name", "completed"),
            ("call-3", "broken", "running"),
            ("call-3", "broken", "failed"),
        ]
    ]
    assert json.loads(gateway.tool_reports[1].output_json) == {"rolls": [4, 4]}
    assert gateway.tool_reports[5].error == "dice fell off the table"


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

    tools = convert_to_agno_tools({STAGES: catalog_tool(STAGES), REGION: catalog_tool(REGION)})
    stages, region = (t.name for t in tools)
    # The same alias in every adapter and in the TypeScript SDK.
    assert stages == "customer_relationship_hub_list_open_opportunities_for_o_d9926af9"
    assert len(region) == 64 and region != stages
    await FunctionCall(function=tools[1], arguments={}, call_id="call-1").aexecute()
    assert calls == [REGION]
