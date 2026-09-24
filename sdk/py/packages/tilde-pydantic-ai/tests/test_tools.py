import pytest
from pydantic_ai import Agent
from pydantic_ai.messages import ModelResponse, TextPart, ToolCallPart
from pydantic_ai.models.function import AgentInfo, FunctionModel

from tilde import ChannelTool, Tool
from tilde_pydantic_ai import convert_to_pydantic_ai_tools

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
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_pydantic_ai_tools({"x" * 65: source})
