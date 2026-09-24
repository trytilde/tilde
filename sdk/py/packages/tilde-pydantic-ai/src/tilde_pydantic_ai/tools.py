"""Expose Tilde channel tools to Pydantic AI without altering provider schemas or authorization."""

from __future__ import annotations

import re
from collections.abc import Mapping
from typing import Any

from pydantic_ai import RunContext
from pydantic_ai.tools import Tool

from tilde import ChannelTool

_INVALID = re.compile(r"[^a-zA-Z0-9_-]")


def convert_to_pydantic_ai_tools(
    channels: Mapping[str, ChannelTool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[Tool[Any]]:
    """Preserve provider schemas and forward the model's tool-call id to Tilde's audited execution.

    ``instructions`` overrides provider descriptions by channel tool name; ``prefix`` namespaces
    model tool names when combining several channel collections. Pass the result as ``tools=``
    to ``Agent``.
    """
    result: list[Tool[Any]] = []
    seen: set[str] = set()
    for name, channel in channels.items():
        key = _INVALID.sub("_", f"{prefix}{name}")
        if not key or len(key) > 64 or key in seen:
            raise ValueError("Conflicting or invalid model tool names")
        seen.add(key)
        description = (
            instructions[name] if instructions and name in instructions else channel.description
        )
        result.append(
            Tool.from_schema(
                _execute(channel),
                name=key,
                description=description,
                json_schema=channel.input_schema,
                takes_ctx=True,
            )
        )
    return result


def _execute(channel: ChannelTool):
    # Pydantic AI sets RunContext.tool_call_id to the model's call id before invoking a tool
    # function that takes the context; the schema's fields arrive as keyword arguments.
    async def execute(ctx: RunContext[Any], /, **arguments: Any) -> Any:
        return await channel.execute(arguments, tool_call_id=ctx.tool_call_id)

    return execute
