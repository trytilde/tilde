"""Expose Tilde channel tools to Agno with provider schemas and audited tool-call ids intact."""

from __future__ import annotations

import copy
import re
from collections.abc import Mapping
from typing import Any

from agno.tools.function import Function, FunctionCall

from tilde import ChannelTool

_INVALID = re.compile(r"[^a-zA-Z0-9_-]")


def convert_to_agno_tools(
    channels: Mapping[str, ChannelTool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[Function]:
    """Build Agno ``Function`` tools that keep provider schemas and execution ids.

    ``skip_entrypoint_processing`` stops Agno from replacing the provider JSON schema with one
    introspected from the entrypoint. Agno injects the running ``FunctionCall`` into an
    entrypoint parameter named ``fc``; its ``call_id`` is the model's tool-call id and is
    forwarded to Tilde's audited execution. A provider schema with a property named ``fc`` is
    rejected here because Agno would drop that argument before it reached the channel.
    ``instructions`` overrides descriptions by channel name; ``prefix`` namespaces model tool
    names when combining collections.
    """
    result: list[Function] = []
    names: set[str] = set()
    for name, channel in channels.items():
        key = _INVALID.sub("_", f"{prefix}{name}")
        if not key or len(key) > 64 or key in names:
            raise ValueError("Conflicting or invalid model tool names")
        names.add(key)
        if "fc" in (channel.input_schema.get("properties") or {}):
            raise ValueError(
                f"{name}: Agno reserves the `fc` argument for its injected FunctionCall"
            )
        description = (
            instructions[name]
            if instructions is not None and name in instructions
            else channel.description
        )
        result.append(
            Function(
                name=key,
                description=description,
                parameters=copy.deepcopy(channel.input_schema),
                entrypoint=_entrypoint(channel),
                skip_entrypoint_processing=True,
            )
        )
    return result


def _entrypoint(channel: ChannelTool):
    async def execute(fc: FunctionCall, **arguments: Any) -> Any:
        return await channel.execute(arguments, tool_call_id=fc.call_id or None)

    return execute
