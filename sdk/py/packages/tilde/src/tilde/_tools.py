"""Tools in ``ctx.tools``: provider tools published for the invocation (descriptions and schemas
come from Tilde) and SDK lifecycle helpers. The agent's bundled tools are its own framework tools,
published by each adapter's ``with_tilde_tools``."""

from __future__ import annotations

import hashlib
import re
import uuid
from collections.abc import AsyncIterable, Awaitable, Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, Literal

ExecuteFn = Callable[[Any, str], Awaitable[Any]]
#: How calls of one of the agent's own tools show in end-user chats; traces keep full detail.
ToolDisplay = Literal["full", "summary", "hidden"]
StreamFn = Callable[[Any, AsyncIterable[Any], str], Awaitable[Any]]


@dataclass(slots=True, frozen=True)
class ToolAnnotations:
    """Hints for ordering, confirming or parallelising calls; never authorization."""

    read_only: bool = False
    destructive: bool = False
    idempotent: bool = False
    open_world: bool = False


@dataclass(slots=True)
class Tool:
    """One tool in ``ctx.tools``, or the published definition of a bundled tool."""

    description: str
    input_schema: dict[str, Any]
    _execute: ExecuteFn = field(repr=False)
    provider_id: str | None = None
    chunk_schema: dict[str, Any] | None = None
    _stream: StreamFn | None = field(default=None, repr=False)
    output_schema: dict[str, Any] | None = None
    #: Short label for transcripts.
    summary: str | None = None
    annotations: ToolAnnotations | None = None
    #: Returns a ticket at once; the outcome arrives later as new input and through tools.result.
    background: bool = False
    display: ToolDisplay = "full"

    async def execute(self, input: Any, *, tool_call_id: str | None = None) -> Any:
        """Send completed input; ``tool_call_id`` is the framework's call ID for auditing."""
        return await self._execute(input, tool_call_id or str(uuid.uuid4()))

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


@dataclass(slots=True, frozen=True)
class BundledOptions:
    """Tilde metadata for one of the agent's own framework tools, overriding what the framework
    declares. ``display`` is how its calls show in end-user chats: with input and output, only
    the summary, or hidden; traces keep full detail."""

    summary: str | None = None
    display: ToolDisplay = "full"
    annotations: ToolAnnotations | None = None


@dataclass(slots=True, frozen=True)
class BundledTools:
    """An agent's own framework tools with their Tilde options, from ``define_tools``."""

    tools: list[Any]
    options: dict[str, BundledOptions]


def define_tools(
    tools: Sequence[Any], *, options: Mapping[str, BundledOptions] | None = None
) -> BundledTools:
    """Declare the agent's bundled tools: its framework's own tools (functions, ``@tool``s,
    ``FunctionTool``s, ...) with Tilde options by tool name. Pass the result to the adapter's
    per-invocation helper as ``bundled`` (or its parts to ``with_tilde_tools``); at module level,
    ``tilde deploy`` declares the tools with the deployment."""
    return BundledTools(list(tools), dict(options or {}))


@dataclass(slots=True, frozen=True)
class ToolSpec:
    """One of the agent's framework tools as Tilde publishes it per invocation and ``tilde deploy``
    declares it, with its Tilde options applied. Each adapter's ``describe_tool`` builds it."""

    name: str
    description: str
    input_schema: dict[str, Any]
    output_schema: dict[str, Any] | None = None
    summary: str | None = None
    annotations: ToolAnnotations | None = None
    display: ToolDisplay = "full"


def tool_spec(
    name: str,
    description: str,
    input_schema: dict[str, Any],
    *,
    output_schema: dict[str, Any] | None = None,
    annotations: ToolAnnotations | None = None,
    options: BundledOptions | None = None,
) -> ToolSpec:
    """What the framework declares, overridden by ``options``."""
    options = options or BundledOptions()
    return ToolSpec(
        name=name,
        description=description,
        input_schema=input_schema,
        output_schema=output_schema,
        summary=options.summary,
        annotations=options.annotations or annotations,
        display=options.display,
    )


def bundled_tool(spec: ToolSpec, execute: ExecuteFn) -> Tool:
    """Internal: the definition adapters publish for a framework tool. ``execute`` runs it for a
    ``tools.execute`` routed here and must audit the call itself, exactly as a direct call is."""
    return Tool(
        description=spec.description,
        input_schema=spec.input_schema,
        _execute=execute,
        output_schema=spec.output_schema,
        summary=spec.summary,
        annotations=spec.annotations,
        display=spec.display,
    )


_MODEL_INVALID = re.compile(r"[^a-zA-Z0-9_-]")


def model_tool_name(name: str, invalid: re.Pattern[str] = _MODEL_INVALID) -> str:
    """The name a framework shows the model for a Tilde tool: ``name`` with characters matching
    ``invalid`` (the framework's disallowed characters) replaced by ``_``. Catalog names
    (``{source}.{tool}``) can exceed the 64 characters models accept, so a longer result is
    truncated and suffixed with a hash of ``name``, keeping it stable and distinct from other
    long names sharing its start. Adapters bind each alias to its catalog tool when converting.
    Matches ``modelToolName`` in the TypeScript SDK."""
    key = invalid.sub("_", name)
    if len(key) <= 64:
        return key
    return f"{key[:55]}_{hashlib.sha256(name.encode()).hexdigest()[:8]}"


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
