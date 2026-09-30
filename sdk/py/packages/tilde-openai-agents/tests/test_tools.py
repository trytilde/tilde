import json
from typing import Any

import pytest
from agents import function_tool
from agents.tool_context import ToolContext
from fake_gateway import invocation_context

from tilde import BundledOptions, Tool, tool_id
from tilde.types.v1 import chat_pb2 as types
from tilde_openai_agents import convert_to_openai_agents_tools, with_tilde_tools

SCHEMA = {"type": "object", "properties": {"value": {"type": "number"}}, "required": ["value"]}


class FakeChannelTool:
    def __init__(self) -> None:
        self.description = "Provider instructions"
        self.input_schema = SCHEMA
        self.calls: list[tuple[Any, str | None]] = []

    async def execute(self, input: Any, *, tool_call_id: str | None = None) -> Any:
        self.calls.append((input, tool_call_id))
        return {"ok": True}


async def test_tool_conversion_preserves_schema_overrides_instructions_and_forwards_ids() -> None:
    source = FakeChannelTool()
    tools = convert_to_openai_agents_tools(
        {"customer_action": source},
        prefix="custom_",
        instructions={"customer_action": "Custom model instructions"},
    )
    (tool,) = tools
    assert tool.name == "custom_customer_action"
    assert tool.description == "Custom model instructions"
    assert tool.params_json_schema is SCHEMA
    assert tool.strict_json_schema is False
    ctx = ToolContext(
        context=None,
        tool_name=tool.name,
        tool_call_id="call-123",
        tool_arguments=json.dumps({"value": 3}),
    )
    assert await tool.on_invoke_tool(ctx, ctx.tool_arguments) == '{"ok": true}'
    assert source.calls == [({"value": 3}, "call-123")]


def test_conflicting_names_are_rejected() -> None:
    source = FakeChannelTool()
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_openai_agents_tools({"a.b": source, "a_b": source})


@function_tool
def roll_dice(count: int = 1) -> dict[str, list[int]]:
    """Roll six-sided dice."""
    return {"rolls": [4] * count}


@function_tool
def lenient() -> str:
    """Fails; the Agents SDK hands the error text to the model."""
    raise RuntimeError("dice fell off the table")


@function_tool(failure_error_function=None)
def strict() -> str:
    """Fails and fails the run."""
    raise RuntimeError("dice fell off the table")


def tool_context(name: str, call: str, arguments: dict) -> ToolContext:
    return ToolContext(
        context=None, tool_name=name, tool_call_id=call, tool_arguments=json.dumps(arguments)
    )


async def test_native_tools_are_published_audited_once_and_routed_through_tools_execute():
    async with invocation_context() as (ctx, gateway):
        tools = await with_tilde_tools(
            ctx,
            [roll_dice, lenient, strict],
            options={"roll_dice": BundledOptions(summary="Rolled dice", display="summary")},
        )
        by_name = {tool.name: tool for tool in tools}
        assert "roll_dice" not in ctx.agent_tools
        assert {"sendMessage", "tools_execute", "roll_dice"} <= set(by_name)

        # The Agents SDK's own call path: on_invoke_tool with the model's call id.
        dice = by_name["roll_dice"]
        ran = await dice.on_invoke_tool(
            tool_context("roll_dice", "call-1", {"count": 2}), '{"count": 2}'
        )
        assert ran == {"rolls": [4, 4]}
        routed = {"name": "roll_dice", "input": {"count": 1}}
        execute = by_name["tools_execute"]
        output = await execute.on_invoke_tool(
            tool_context("tools_execute", "call-2", routed), json.dumps(routed)
        )
        assert json.loads(output) == {"rolls": [4]}
        handled = await by_name["lenient"].on_invoke_tool(tool_context("lenient", "call-3", {}), "")
        assert "dice fell off the table" in handled
        with pytest.raises(RuntimeError, match="fell off"):
            await by_name["strict"].on_invoke_tool(tool_context("strict", "call-4", {}), "")

    _, (published, *_) = gateway.bundled_registrations
    assert (published.name, published.description) == ("roll_dice", "Roll six-sided dice.")
    assert (published.summary, published.display) == ("Rolled dice", types.TOOL_DISPLAY_SUMMARY)
    assert json.loads(published.input_schema_json)["properties"]["count"]["type"] == "integer"
    assert gateway.tool_calls == []
    assert [(c.id, c.name, c.status) for c in gateway.tool_reports] == [
        (tool_id(ctx.invocation_id, call, name), name, status)
        for call, name, status in [
            ("call-1", "roll_dice", "running"),
            ("call-1", "roll_dice", "completed"),
            ("call-2", "roll_dice", "running"),
            ("call-2", "roll_dice", "completed"),
            # The documented caveat: a handled failure is audited as completed.
            ("call-3", "lenient", "running"),
            ("call-3", "lenient", "completed"),
            ("call-4", "strict", "running"),
            ("call-4", "strict", "failed"),
        ]
    ]
    assert gateway.tool_reports[1].summary == "Rolled dice"
    assert gateway.tool_reports[7].error == "dice fell off the table"


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

    tools = convert_to_openai_agents_tools(
        {STAGES: catalog_tool(STAGES), REGION: catalog_tool(REGION)}
    )
    stages, region = (t.name for t in tools)
    # The same alias in every adapter and in the TypeScript SDK.
    assert stages == "customer_relationship_hub_list_open_opportunities_for_o_d9926af9"
    assert len(region) == 64 and region != stages
    context = ToolContext(
        context=None, tool_name=region, tool_call_id="call-1", tool_arguments="{}"
    )
    await tools[1].on_invoke_tool(context, "{}")
    assert calls == [REGION]
