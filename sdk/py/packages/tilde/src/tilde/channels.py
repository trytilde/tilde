"""Group only the tools published for this invocation.

The API remains the authorization boundary.
"""

from __future__ import annotations

import json
import re
import uuid
from collections.abc import AsyncIterable, Iterator, Mapping
from typing import Any

from tilde._tools import Tool, ToolCatalog
from tilde.types.v1.chat_pb2 import ChannelBinding, Message

_CHANNEL = re.compile(r"^channel_([a-f0-9]{32})\.(.+)$", re.IGNORECASE)


def _snake_to_camel(name: str) -> str:
    head, *rest = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


class ChannelTool:
    """Callable programmatically; its provider-authored descriptor can also be given to a model."""

    def __init__(self, name: str, source: Tool, catalog: ToolCatalog, description: str) -> None:
        self.tool_name = name
        self.description = description
        self.input_schema = source.input_schema
        self.provider_id = source.provider_id
        self.chunk_schema = source.chunk_schema
        self._source = source
        self._catalog = catalog

    def _available(self) -> None:
        if self._catalog.get(self.tool_name) is not self._source:
            raise RuntimeError("Channel tool is no longer available in this invocation")

    async def execute(self, input: Any, *, tool_call_id: str | None = None) -> Any:
        self._available()
        return await self._source.execute(input, tool_call_id=tool_call_id or str(uuid.uuid4()))

    async def __call__(self, input: Any, *, tool_call_id: str | None = None) -> Any:
        return await self.execute(input, tool_call_id=tool_call_id)

    @property
    def can_stream(self) -> bool:
        return self._source.can_stream

    async def stream(
        self, input: Any, chunks: AsyncIterable[Any], *, tool_call_id: str | None = None
    ) -> Any:
        self._available()
        return await self._source.stream(
            input, chunks, tool_call_id=tool_call_id or str(uuid.uuid4())
        )


class ChannelTools(Mapping[str, ChannelTool]):
    """Provider tools of one connection, keyed by their published short names (``sendMessage``).

    Attribute access accepts snake_case (``tools.send_message``) and returns None for tools
    the agent cannot use, mirroring the optional typed collections of the TypeScript SDK.
    """

    def __init__(self, tools: dict[str, ChannelTool]) -> None:
        self._tools = tools

    def __getitem__(self, key: str) -> ChannelTool:
        return self._tools[key]

    def __iter__(self) -> Iterator[str]:
        return iter(self._tools)

    def __len__(self) -> int:
        return len(self._tools)

    def __getattr__(self, name: str) -> ChannelTool | None:
        if name.startswith("_"):
            raise AttributeError(name)
        return self._tools.get(name) or self._tools.get(_snake_to_camel(name))

    def __repr__(self) -> str:
        return f"ChannelTools({sorted(self._tools)})"


class Channels:
    """``ctx.channel``: the inbound connection's tools plus explicit provider/connection lookups."""

    def __init__(
        self, catalog: ToolCatalog, binding: ChannelBinding | None, incoming: Message | None
    ) -> None:
        self._catalog = catalog
        self._binding = binding
        self._incoming = incoming
        self._wrapped: dict[int, ChannelTool] = {}

    @staticmethod
    def _key(connection_id: str) -> str:
        return connection_id.replace("-", "").lower()

    def _entries(self) -> list[tuple[str, Tool, str, str]]:
        entries = []
        for name, tool in self._catalog.items():
            match = _CHANNEL.match(name)
            if match and tool.provider_id:
                entries.append((name, tool, match.group(1).lower(), match.group(2)))
            elif tool.provider_id == "native":
                entries.append((name, tool, "native", name))
        return entries

    def _wrap(self, name: str, source: Tool) -> ChannelTool:
        cached = self._wrapped.get(id(source))
        if cached is not None and cached._source is source:
            return cached
        description = source.description
        if (
            self._binding
            and name.startswith(f"channel_{self._key(self._binding.connection_id)}.")
            and self._incoming is not None
            and self._incoming.delivery.external_message_id
        ):
            description += (
                f" Inbound message reference: {self._incoming.delivery.external_message_id}."
            )
        wrapped = ChannelTool(name, source, self._catalog, description)
        self._wrapped[id(source)] = wrapped
        return wrapped

    def for_connection(self, connection_id: str) -> ChannelTools:
        key = self._key(connection_id)
        return ChannelTools(
            {
                short: self._wrap(name, tool)
                for name, tool, cid, short in self._entries()
                if cid == key
            }
        )

    def provider(self, provider_id: str, connection_id: str | None = None) -> ChannelTools | None:
        candidates = [e for e in self._entries() if e[1].provider_id == provider_id]
        ids = list(dict.fromkeys(e[2] for e in candidates))
        if connection_id:
            return self.for_connection(connection_id) if self._key(connection_id) in ids else None
        if self._binding and self._key(self._binding.connection_id) in ids:
            return self.for_connection(self._binding.connection_id)
        if len(ids) > 1:
            raise RuntimeError(
                f"Multiple {provider_id} connections are available; use channel.for_connection(id)"
            )
        return self.for_connection(ids[0]) if ids else None

    @property
    def current(self) -> ChannelTools:
        """Only the inbound connection's tools; never another connection's."""
        return self.for_connection(self._binding.connection_id if self._binding else "native")

    @property
    def native(self) -> ChannelTools | None:
        return self.provider("native")

    @property
    def slack(self) -> ChannelTools | None:
        return self.provider("slack")

    @property
    def github(self) -> ChannelTools | None:
        return self.provider("github")

    @property
    def agentmail(self) -> ChannelTools | None:
        return self.provider("agentmail")

    @property
    def linq(self) -> ChannelTools | None:
        return self.provider("linq")

    @property
    def whatsapp(self) -> ChannelTools | None:
        return self.provider("whatsapp")

    @property
    def telnyx_whatsapp(self) -> ChannelTools | None:
        return self.provider("telnyx")

    def connections(self) -> list[dict[str, str]]:
        seen: dict[str, dict[str, str]] = {}
        for _name, tool, cid, _short in self._entries():
            if cid in seen:
                continue
            connection = (
                cid
                if cid == "native"
                else f"{cid[:8]}-{cid[8:12]}-{cid[12:16]}-{cid[16:20]}-{cid[20:]}"
            )
            seen[cid] = {"connection_id": connection, "provider_id": tool.provider_id or ""}
        return list(seen.values())

    async def call_channel_tool(
        self, name: str, serialized_args: str, *, tool_call_id: str | None = None
    ) -> Any:
        """Call a custom provider tool by its full published name with JSON arguments."""
        tool = self._catalog.get(name)
        if tool is None or not tool.provider_id:
            raise RuntimeError("Channel tool is not available in this invocation")
        try:
            input = json.loads(serialized_args)
        except ValueError as error:
            raise ValueError("Channel tool arguments must be valid JSON") from error
        return await self._wrap(name, tool)(input, tool_call_id=tool_call_id)
