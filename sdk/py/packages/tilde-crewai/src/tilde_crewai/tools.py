"""Expose Tilde channel tools to CrewAI with provider descriptions and schemas intact."""

from __future__ import annotations

import asyncio
import copy
import json
import re
from collections.abc import Mapping
from typing import Any

from crewai.tools import BaseTool
from crewai.utilities.string_utils import sanitize_tool_name
from pydantic import BaseModel, PrivateAttr

from tilde import ChannelTool

_INVALID = re.compile(r"[^a-zA-Z0-9_-]")


class ChannelCrewTool(BaseTool):
    """A channel tool executed on the agent's event loop from wherever CrewAI calls it."""

    _channel: ChannelTool = PrivateAttr()
    _loop: asyncio.AbstractEventLoop = PrivateAttr()

    def _run(self, **arguments: Any) -> str:
        # CrewAI's executor calls tools synchronously on a worker thread, while channel
        # execution belongs to the invocation's event loop.
        try:
            running = asyncio.get_running_loop()
        except RuntimeError:
            running = None
        if running is self._loop:
            raise RuntimeError("Channel tools block when run on the event loop; await arun()")
        future = asyncio.run_coroutine_threadsafe(self._arun(**arguments), self._loop)
        return future.result()

    async def _arun(self, **arguments: Any) -> str:
        # CrewAI does not pass the model's tool-call id to tools; the core generates one.
        result = await self._channel.execute(arguments)
        # CrewAI renders results with str(); JSON reads better to the model than a Python repr.
        return result if isinstance(result, str) else json.dumps(result, default=str)


def convert_to_crewai_tools(
    channels: Mapping[str, ChannelTool],
    *,
    instructions: Mapping[str, str] | None = None,
    prefix: str = "",
) -> list[BaseTool]:
    """Build CrewAI tools that keep provider descriptions and JSON schemas.

    Call this on the invocation's event loop: the loop is captured so that CrewAI's worker
    threads can execute channel tools on it. CrewAI reads a tool's parameters from
    ``args_schema.model_json_schema()``, so the schema model returns the provider schema rather
    than one introspected from fields; having no fields, it also passes arguments through
    unvalidated. CrewAI then applies its own strict-mode pass and name normalization (see the
    README). ``instructions`` overrides descriptions by channel name; ``prefix`` namespaces
    model tool names when combining collections.
    """
    loop = asyncio.get_running_loop()
    result: list[BaseTool] = []
    names: set[str] = set()
    for name, channel in channels.items():
        key = _INVALID.sub("_", f"{prefix}{name}")
        # CrewAI lowercases and snake_cases names for the model and would silently suffix a
        # duplicate, so conflicts are checked on the name the model sees.
        model_name = sanitize_tool_name(key)
        if not key or len(key) > 64 or not model_name or model_name in names:
            raise ValueError("Conflicting or invalid model tool names")
        names.add(model_name)
        description = (
            instructions[name]
            if instructions is not None and name in instructions
            else channel.description
        )
        tool = ChannelCrewTool(
            name=key, description=description, args_schema=_schema_model(channel.input_schema)
        )
        tool._channel = channel
        tool._loop = loop
        result.append(tool)
    return result


def _schema_model(schema: dict[str, Any]) -> type[BaseModel]:
    class ChannelToolInput(BaseModel):
        @classmethod
        def model_json_schema(cls, *args: Any, **kwargs: Any) -> dict[str, Any]:
            return copy.deepcopy(schema)

    return ChannelToolInput
