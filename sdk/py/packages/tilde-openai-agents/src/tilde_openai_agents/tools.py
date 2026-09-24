"""Channel tools as ``agents.FunctionTool`` instances with audited, id-preserving execution."""

from __future__ import annotations

import json
import re
from collections.abc import Mapping

from agents import FunctionTool
from agents.tool_context import ToolContext

from tilde import ChannelTool

_INVALID = re.compile(r"[^a-zA-Z0-9_-]")


def convert_to_openai_agents_tools(
    channels: Mapping[str, ChannelTool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[FunctionTool]:
    """Preserve provider descriptions and schemas; forward the model's call id to Tilde.

    ``instructions`` overrides descriptions by channel tool name without touching schemas or
    authorization. ``prefix`` namespaces model tool names when combining collections.
    """
    tools: list[FunctionTool] = []
    names: set[str] = set()
    for name, channel in channels.items():
        key = _INVALID.sub("_", f"{prefix}{name}")
        if not key or len(key) > 64 or key in names:
            raise ValueError("Conflicting or invalid model tool names")
        names.add(key)
        description = (
            instructions[name] if instructions and name in instructions else channel.description
        )
        tools.append(
            FunctionTool(
                name=key,
                description=description,
                params_json_schema=channel.input_schema,
                on_invoke_tool=_invoker(channel),
                strict_json_schema=False,
            )
        )
    return tools


def _invoker(channel: ChannelTool):
    async def on_invoke_tool(ctx: ToolContext, input_json: str) -> str:
        arguments = json.loads(input_json) if input_json.strip() else {}
        result = await channel.execute(arguments, tool_call_id=ctx.tool_call_id)
        return json.dumps(result)

    return on_invoke_tool
