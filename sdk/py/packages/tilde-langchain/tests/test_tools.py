import asyncio

import pytest
from langchain_core.messages import ToolMessage
from langchain_core.utils.function_calling import convert_to_openai_tool

from tilde import ChannelTool, Tool
from tilde_langchain import convert_to_langchain_tools

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
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_langchain_tools({"x" * 65: source})
