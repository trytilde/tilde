import json
from types import SimpleNamespace

import pytest
from fake_gateway import invocation_context
from pydantic_ai import Agent
from pydantic_ai.messages import ModelResponse, TextPart, ToolCallPart
from pydantic_ai.models.function import AgentInfo, FunctionModel
from pydantic_ai.tools import Tool as PydanticTool

from tilde import BundledOptions, ChannelTool, Tool, tool_id
from tilde.types.v1 import chat_pb2 as types
from tilde_pydantic_ai import convert_to_pydantic_ai_tools, with_tilde_tools

SCHEMA = {"type": "object", "properties": {"value": {"type": "number"}}, "required": ["value"]}


def channel_tool(calls: list, name: str = "customer_action") -> ChannelTool:
    async def execute(input, tool_call_id):
        calls.append((input, tool_call_id))
        return {"ok": True}

    source = Tool(description="Provider instructions", input_schema=SCHEMA, _execute=execute)
    return ChannelTool(name, source, {name: source}, "Provider instructions")


async def test_conversion_preserves_schema_overrides_instructions_and_forwards_call_ids():
    calls = []
    tools = convert_to_pydantic_ai_tools(
        {"customer_action": channel_tool(calls)},
        prefix="custom_",
        instructions={"customer_action": "Custom model instructions"},
    )
    [tool] = tools
    assert tool.name == "custom_customer_action"
    assert tool.tool_def.description == "Custom model instructions"
    assert tool.tool_def.parameters_json_schema == SCHEMA

    def model(messages, info: AgentInfo) -> ModelResponse:
        assert [t.parameters_json_schema for t in info.function_tools] == [SCHEMA]
        if len(messages) == 1:
            return ModelResponse(
                parts=[
                    ToolCallPart(
                        tool_name="custom_customer_action",
                        args={"value": 3},
                        tool_call_id="call-123",
                    )
                ]
            )
        return ModelResponse(parts=[TextPart("done")])

    agent = Agent(FunctionModel(model), tools=tools)
    result = await agent.run("go")
    assert result.output == "done"
    assert calls == [({"value": 3}, "call-123")]


def test_conflicting_names_are_rejected():
    source = channel_tool([])
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_pydantic_ai_tools({"a.b": source, "a_b": source})


async def test_native_tools_are_published_audited_once_and_routed_through_tools_execute():
    def roll_dice(count: int = 1) -> dict[str, list[int]]:
        """Roll six-sided dice."""
        return {"rolls": [4] * count}

    async def broken() -> str:
        """Always fails."""
        raise RuntimeError("dice fell off the table")

    def model(messages, info: AgentInfo) -> ModelResponse:
        calls = {
            1: ("roll_dice", {"count": 2}, "call-1"),
            # Found by tools.search elsewhere: Tilde's tools.execute runs it here.
            3: ("tools_execute", {"name": "roll_dice", "input": {"count": 1}}, "call-2"),
            5: ("broken", {}, "call-3"),
        }
        name, args, call = calls[len(messages)]
        return ModelResponse(parts=[ToolCallPart(tool_name=name, args=args, tool_call_id=call)])

    async with invocation_context() as (ctx, gateway):
        toolset = await with_tilde_tools(
            ctx,
            [
                PydanticTool(roll_dice, metadata={"tilde": BundledOptions(summary="Rolled dice")}),
                broken,
            ],
            options={"broken": BundledOptions(summary="Broke", display="hidden")},
        )
        assert "roll_dice" not in ctx.agent_tools
        with pytest.raises(RuntimeError, match="fell off"):
            await Agent(FunctionModel(model), toolsets=[toolset]).run("go")

    _, (dice, failing) = gateway.bundled_registrations
    assert (dice.name, dice.description, dice.summary) == (
        "roll_dice",
        "Roll six-sided dice.",
        "Rolled dice",
    )
    assert json.loads(dice.input_schema_json)["properties"]["count"]["type"] == "integer"
    assert json.loads(dice.output_schema_json)["type"] == "object"
    assert (failing.name, failing.summary, failing.display) == (
        "broken",
        "Broke",
        types.TOOL_DISPLAY_HIDDEN,
    )
    assert gateway.tool_calls == []
    assert [(c.id, c.name, c.status, c.summary) for c in gateway.tool_reports] == [
        (tool_id(ctx.invocation_id, call, name), name, status, summary)
        for call, name, status, summary in [
            ("call-1", "roll_dice", "running", "Rolled dice"),
            ("call-1", "roll_dice", "completed", "Rolled dice"),
            ("call-2", "roll_dice", "running", "Rolled dice"),
            ("call-2", "roll_dice", "completed", "Rolled dice"),
            ("call-3", "broken", "running", "Broke"),
            ("call-3", "broken", "failed", "Broke"),
        ]
    ]
    assert json.loads(gateway.tool_reports[1].output_json) == {"rolls": [4, 4]}
    assert gateway.tool_reports[5].error == "dice fell off the table"


async def test_with_no_tools_unpublishes_the_last_bundled_tool():
    def roll_dice(count: int) -> dict[str, list[int]]:
        """Roll six-sided dice."""
        return {"rolls": [4] * count}

    async with invocation_context() as (ctx, gateway):
        await with_tilde_tools(ctx, [roll_dice])
        (old,) = gateway.bundled_registrations[-1]
        tools = await with_tilde_tools(ctx, [])
        assert old.name not in set(tools.wrapped.tools)
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

    tools = convert_to_pydantic_ai_tools(
        {STAGES: catalog_tool(STAGES), REGION: catalog_tool(REGION)}
    )
    stages, region = (t.name for t in tools)
    # The same alias in every adapter and in the TypeScript SDK.
    assert stages == "customer_relationship_hub_list_open_opportunities_for_o_d9926af9"
    assert len(region) == 64 and region != stages
    await tools[1].function(SimpleNamespace(tool_call_id="call-1"))
    assert calls == [REGION]
