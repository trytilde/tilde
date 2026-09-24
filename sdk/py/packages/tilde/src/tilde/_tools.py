"""Local wrappers for invocation-scoped tools. Descriptions and schemas come from Tilde."""

from __future__ import annotations

import hashlib
from collections.abc import AsyncIterable, Awaitable, Callable
from dataclasses import dataclass, field
from typing import Any

ExecuteFn = Callable[[Any, str], Awaitable[Any]]
StreamFn = Callable[[Any, AsyncIterable[Any], str], Awaitable[Any]]


@dataclass(slots=True)
class Tool:
    """One SDK-local tool: a lifecycle helper or a provider tool published for this invocation."""

    description: str
    input_schema: dict[str, Any]
    _execute: ExecuteFn = field(repr=False)
    provider_id: str | None = None
    chunk_schema: dict[str, Any] | None = None
    _stream: StreamFn | None = field(default=None, repr=False)

    async def execute(self, input: Any, *, tool_call_id: str) -> Any:
        """Send completed input; ``tool_call_id`` is the framework's call ID for auditing."""
        return await self._execute(input, tool_call_id)

    @property
    def can_stream(self) -> bool:
        return self._stream is not None

    async def stream(self, input: Any, chunks: AsyncIterable[Any], *, tool_call_id: str) -> Any:
        """Send incremental input in the provider's ``chunk_schema`` format.

        A framework must feed chunks explicitly; registering a tool never streams model tokens.
        """
        if self._stream is None:
            raise ValueError("This tool does not accept incremental input")
        return await self._stream(input, chunks, tool_call_id)


ToolCatalog = dict[str, Tool]


def tool_id(invocation: str, call: str, name: str) -> str:
    """Stable UUID derived from the invocation and the framework's tool-call ID."""
    if not call:
        raise ValueError("Tool call ID is required")
    h = hashlib.sha256(f"{invocation}:{call}:{name}".encode()).hexdigest()
    return f"{h[:8]}-{h[8:12]}-4{h[13:16]}-a{h[17:20]}-{h[20:32]}"


def as_object(input: Any) -> dict[str, Any]:
    if not isinstance(input, dict):
        raise ValueError("Tool input must be an object")
    return input


def as_string(input: dict[str, Any], key: str) -> str:
    value = input.get(key)
    if not isinstance(value, str):
        raise ValueError(f"{key} must be a string")
    return value
