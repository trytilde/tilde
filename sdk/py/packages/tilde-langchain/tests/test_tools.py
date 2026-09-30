import asyncio
import json
from typing import Annotated

import pytest
from fake_gateway import invocation_context
from langchain_core.messages import AIMessage, ToolMessage
from langchain_core.tools import InjectedToolCallId, ToolException, tool
from langchain_core.utils.function_calling import convert_to_openai_tool
from langgraph.graph import START, MessagesState, StateGraph
from langgraph.prebuilt import ToolNode

from tilde import BundledOptions, ChannelTool, Tool, tool_id
from tilde.types.v1 import chat_pb2 as types
from tilde_langchain import convert_to_langchain_tools, with_tilde_tools

SCHEMA = {"type": "object", "properties": {"value": {"type": "number"}}, "required": ["value"]}


def channel_tool(name: str, execute) -> ChannelTool:
    source = Tool(description="Provider instructions", input_schema=SCHEMA, _execute=execute)
    return ChannelTool(name, source, {name: source}, source.description)


async def test_tool_conversion_preserves_schema_overrides_instructions_and_forwards_ids() -> None:
    calls: list[tuple[object, str]] = []

    async def execute(input, tool_call_id):
        calls.append((input, tool_call_id))
        return {"ok": True}

    [tool] = convert_to_langchain_tools(
        {"customer_action": channel_tool("customer_action", execute)},
        prefix="custom_",
        instructions={"customer_action": "Custom model instructions"},
    )
    assert tool.name == "custom_customer_action"
    assert tool.description == "Custom model instructions"
    assert convert_to_openai_tool(tool)["function"]["parameters"] == SCHEMA

    result = await tool.ainvoke(
        {"name": tool.name, "args": {"value": 3}, "id": "call-123", "type": "tool_call"}
    )
    assert isinstance(result, ToolMessage)
    assert result.tool_call_id == "call-123"
    assert calls == [({"value": 3}, "call-123")]

    # Without a ToolCall there is no model call id; the core substitutes a random UUID.
    assert await tool.ainvoke({"value": 4}) == {"ok": True}
    _, generated = calls[1]
    assert generated and generated != "call-123"


async def test_tool_execution_is_cancellable() -> None:
    async def slow(input, tool_call_id):
        await asyncio.sleep(10)

    [tool] = convert_to_langchain_tools({"slow": channel_tool("slow", slow)})
    task = asyncio.create_task(tool.ainvoke({"value": 1}))
    await asyncio.sleep(0)
    task.cancel()
    with pytest.raises(asyncio.CancelledError):
        await task


def test_conflicting_names_are_rejected() -> None:
    async def execute(input, tool_call_id):
        return None

    source = channel_tool("a.b", execute)
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_langchain_tools({"a.b": source, "a_b": source})


@tool
def roll_dice(count: int, tool_call_id: Annotated[str, InjectedToolCallId]) -> dict:
    """Roll six-sided dice."""
    return {"rolls": [4] * count, "call": tool_call_id}


roll_dice.metadata = {"tilde": BundledOptions(summary="Rolled dice", display="summary")}


@tool
def handled() -> str:
    """Fails; LangChain hands the error text to the model."""
    raise ToolException("dice fell off the table")


handled.handle_tool_error = True


@tool
def broken() -> str:
    """Fails and fails the run."""
    raise RuntimeError("dice fell off the table")


def tool_call(name: str, call: str, args: dict) -> dict:
    return {"type": "tool_call", "id": call, "name": name, "args": args}


async def test_native_tools_are_published_audited_once_and_routed_through_tools_execute():
    async with invocation_context() as (ctx, gateway):
        tools = await with_tilde_tools(
            ctx, [roll_dice, handled, broken], options={"broken": BundledOptions(summary="Broke")}
        )
        by_name = {t.name: t for t in tools}
        assert "roll_dice" not in ctx.agent_tools
        assert type(by_name["roll_dice"]) is type(roll_dice)

        # LangGraph's ToolNode (what create_agent runs) injects the call id and calls the tool.
        graph = StateGraph(MessagesState)
        graph.add_node("tools", ToolNode(tools))
        graph.add_edge(START, "tools")
        request = AIMessage(content="", tool_calls=[tool_call("roll_dice", "call-1", {"count": 2})])
        state = await graph.compile().ainvoke({"messages": [request]})
        assert json.loads(state["messages"][-1].content) == {"rolls": [4, 4], "call": "call-1"}

        # Found by tools.search: Tilde's tools.execute runs it here under its own call id.
        routed = {"name": "roll_dice", "input": {"count": 1}}
        result = await by_name["tools_execute"].ainvoke(
            tool_call("tools_execute", "call-2", routed)
        )
        assert json.loads(result.content) == {"rolls": [4], "call": "call-2"}

        await by_name["handled"].ainvoke(tool_call("handled", "call-3", {}))
        with pytest.raises(RuntimeError, match="fell off"):
            await by_name["broken"].ainvoke(tool_call("broken", "call-4", {}))

    _, (dice, *_, failing) = gateway.bundled_registrations
    assert (dice.name, dice.description) == ("roll_dice", "Roll six-sided dice.")
    assert (dice.summary, dice.display) == ("Rolled dice", types.TOOL_DISPLAY_SUMMARY)
    # The schema the model sees: the injected call id is not an argument.
    assert set(json.loads(dice.input_schema_json)["properties"]) == {"count"}
    assert failing.summary == "Broke"
    assert gateway.tool_calls == []
    assert [(c.id, c.name, c.status) for c in gateway.tool_reports] == [
        (tool_id(ctx.invocation_id, call, name), name, status)
        for call, name, status in [
            ("call-1", "roll_dice", "running"),
            ("call-1", "roll_dice", "completed"),
            ("call-2", "roll_dice", "running"),
            ("call-2", "roll_dice", "completed"),
            ("call-3", "handled", "running"),
            ("call-3", "handled", "failed"),
            ("call-4", "broken", "running"),
            ("call-4", "broken", "failed"),
        ]
    ]
    assert gateway.tool_reports[1].summary == "Rolled dice"
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

    tools = convert_to_langchain_tools({STAGES: catalog_tool(STAGES), REGION: catalog_tool(REGION)})
    stages, region = (t.name for t in tools)
    # The same alias in every adapter and in the TypeScript SDK.
    assert stages == "customer_relationship_hub_list_open_opportunities_for_o_d9926af9"
    assert len(region) == 64 and region != stages
    await tools[1].ainvoke({"name": region, "args": {}, "id": "call-1", "type": "tool_call"})
    assert calls == [REGION]
