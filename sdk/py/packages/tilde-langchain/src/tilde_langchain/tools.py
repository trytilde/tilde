"""Expose Tilde channel tools as LangChain tools with the model's tool-call id forwarded."""

from __future__ import annotations

import re
from collections.abc import Mapping
from typing import Any

from langchain_core.tools import BaseTool

from tilde import ChannelTool

_UNSAFE = re.compile(r"[^a-zA-Z0-9_-]")


class ChannelToolAdapter(BaseTool):
    """Async-only LangChain tool over one Tilde channel tool.

    LangChain passes the model's tool-call id into ``BaseTool.arun`` whenever the tool is
    invoked with a ``ToolCall`` dict (which is what LangGraph's ``ToolNode`` and therefore
    ``create_agent`` do). With a JSON-schema ``args_schema`` nothing injects that id into the
    coroutine arguments, so ``_to_args_and_kwargs`` is overridden to carry it into ``_arun``.
    When invoked directly with plain arguments there is no call id; ``ChannelTool.execute``
    then generates a random UUID, so the call is audited but not deduplicated.
    """

    channel: ChannelTool

    model_config = {"arbitrary_types_allowed": True}

    def _to_args_and_kwargs(
        self, tool_input: str | dict[str, Any], tool_call_id: str | None
    ) -> tuple[tuple[str, ...], dict[str, Any]]:
        return (), {"input": tool_input, "tool_call_id": tool_call_id}

    def _run(self, *args: Any, **kwargs: Any) -> Any:
        raise NotImplementedError("Channel tools run on the agent's event loop; use ainvoke")

    async def _arun(self, input: Any, tool_call_id: str | None = None) -> Any:
        return await self.channel.execute(input, tool_call_id=tool_call_id)


def convert_to_langchain_tools(
    channels: Mapping[str, ChannelTool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[BaseTool]:
    """Preserve provider schemas and descriptions; ``instructions`` overrides descriptions
    without touching schemas or authorization, ``prefix`` namespaces model tool names."""
    result: dict[str, BaseTool] = {}
    for name, channel in channels.items():
        key = _UNSAFE.sub("_", f"{prefix}{name}")
        if not key or len(key) > 64 or key in result:
            raise ValueError("Conflicting or invalid model tool names")
        description = (
            instructions[name]
            if instructions is not None and name in instructions
            else channel.description
        )
        result[key] = ChannelToolAdapter(
            name=key,
            description=description,
            args_schema=channel.input_schema,
            channel=channel,
        )
    return list(result.values())
