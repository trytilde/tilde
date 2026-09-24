import pytest
from agno.tools.function import FunctionCall

from tilde_agno import convert_to_agno_tools

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


def test_conflicting_or_invalid_names_are_rejected():
    channel = FakeChannel()
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_agno_tools({"a.b": channel, "a_b": channel})
    with pytest.raises(ValueError, match="Conflicting"):
        convert_to_agno_tools({"x" * 65: channel})


def test_schema_property_named_fc_is_rejected():
    from tilde_agno.tools import convert_to_agno_tools

    class Channel:
        description = "x"
        input_schema = {"type": "object", "properties": {"fc": {"type": "string"}}}

    with pytest.raises(ValueError, match="fc"):
        convert_to_agno_tools({"tool": Channel()})
