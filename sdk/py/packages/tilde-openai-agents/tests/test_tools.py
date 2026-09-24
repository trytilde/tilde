import json
from typing import Any

import pytest
from agents.tool_context import ToolContext

from tilde_openai_agents import convert_to_openai_agents_tools

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
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_openai_agents_tools({"x" * 65: source})
