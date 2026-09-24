"""Server-side ASGI reverse proxy for the Tilde ConnectRPC chat provider.

The Python counterpart of ``@trytilde/chat-proxy``: browser credentials and identity headers
are replaced, cookies stay at the host, cross-origin requests are rejected, streaming bodies
and cancellation pass through without buffering, and redirects never carry credentials.
"""

from __future__ import annotations

import base64
import json
import re
from collections.abc import AsyncIterator, Awaitable, Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any
from urllib.parse import quote, urlsplit

import pyqwest

from tilde.provider.tilde.v1 import chat_pb2

_CHAT = chat_pb2.DESCRIPTOR.services_by_name["ChatService"]
CHAT_SERVICE = _CHAT.full_name
# Only the provider's declared RPC methods are forwarded.
CHAT_METHODS = frozenset(method.name for method in _CHAT.methods)
DEFAULT_BASE_URL = "https://api.trytilde.ai"
_SEGMENT = re.compile(r"^[a-zA-Z0-9_.-]+$")
_CONTENT_TYPE = re.compile(
    r"^application/(?:json|proto|connect\+json|connect\+proto)(?:;|$)", re.IGNORECASE
)
_FORWARDED_REQUEST = (
    "content-type",
    "connect-protocol-version",
    "connect-timeout-ms",
    "traceparent",
    "tracestate",
)
_FORWARDED_RESPONSE = ("content-type", "connect-content-encoding", "connect-accept-encoding")


@dataclass(slots=True)
class ChatAgentConfig:
    agent_id: str
    api_key: str
    base_url: str | None = None
    """Gateway origin. Defaults to the hosted Tilde API."""
    base_url_override: str | None = None
    """Complete agent ingress base URL; replaces both origin and agent prefix."""


@dataclass(slots=True)
class ProxyRequest:
    """What ``resolve_identity`` sees: the ASGI scope plus parsed headers."""

    method: str
    path: str
    headers: Mapping[str, str]
    scope: Mapping[str, Any]


ResolveIdentity = Callable[[ProxyRequest, str], str | None | Awaitable[str | None]]


def identity_header(identity: str) -> str:
    """The native wire identity header ``x-tilde-identity`` is base64url-encoded UTF-8."""
    return base64.urlsafe_b64encode(identity.encode()).decode().rstrip("=")


class ChatProxy:
    """Pure ASGI app. Mount it below ``mount_path`` (default ``/api/chat``)."""

    def __init__(
        self,
        agents: Sequence[ChatAgentConfig],
        *,
        resolve_identity: ResolveIdentity,
        mount_path: str = "/api/chat",
        http_client: pyqwest.Client | None = None,
        trust_forwarded_headers: bool = True,
    ) -> None:
        if not agents or len({a.agent_id for a in agents}) != len(agents):
            raise ValueError("Configure distinct chat agents")
        self._agents: dict[str, tuple[ChatAgentConfig, str]] = {}
        for agent in agents:
            origin = (agent.base_url or DEFAULT_BASE_URL).rstrip("/")
            base = agent.base_url_override or f"{origin}/agents/{quote(agent.agent_id, safe='')}"
            url = urlsplit(base)
            if (
                url.scheme not in ("http", "https")
                or url.username
                or url.password
                or url.query
                or url.fragment
                or not agent.api_key.startswith("tilde_chat_")
            ):
                raise ValueError("Invalid chat provider configuration")
            self._agents[agent.agent_id] = (agent, base.rstrip("/"))
        self._order = [a.agent_id for a in agents]
        self._resolve = resolve_identity
        self._mount = mount_path.rstrip("/")
        self._trust_forwarded = trust_forwarded_headers
        self._http = http_client or pyqwest.Client(
            pyqwest.HTTPTransport(follow_redirects=False, tls_include_system_certs=True)
        )

    async def _headers(self, request: ProxyRequest, agent_id: str) -> dict[str, str] | None:
        identity = self._resolve(request, agent_id)
        if hasattr(identity, "__await__"):
            identity = await identity  # type: ignore[misc]
        if not identity or len(identity) > 256:
            return None
        result = {
            "authorization": f"Bearer {self._agents[agent_id][0].api_key}",
            "x-tilde-identity": identity_header(identity),
        }
        for name in _FORWARDED_REQUEST:
            value = request.headers.get(name)
            if value:
                result[name] = value
        return result

    async def __call__(self, scope, receive, send) -> None:
        if scope["type"] == "lifespan":
            while True:
                message = await receive()
                if message["type"] == "lifespan.startup":
                    await send({"type": "lifespan.startup.complete"})
                elif message["type"] == "lifespan.shutdown":
                    await send({"type": "lifespan.shutdown.complete"})
                    return
        if scope["type"] != "http":
            raise RuntimeError("Chat proxy only serves HTTP")
        headers = {
            k.decode("latin-1").lower(): v.decode("latin-1") for k, v in scope.get("headers", [])
        }
        request = ProxyRequest(
            method=scope["method"], path=scope["path"], headers=headers, scope=scope
        )
        try:
            await self._handle(request, receive, send)
        except Exception:  # noqa: BLE001 - never expose upstream details
            await _failure(send, 502, "unavailable", "Chat request failed")

    async def _handle(self, request: ProxyRequest, receive, send) -> None:
        origin = request.headers.get("origin")
        if origin and origin != _origin(request, self._trust_forwarded):
            return await _failure(send, 403, "permission_denied", "Origin is not allowed")
        root = request.scope.get("root_path", "") or ""
        full = (
            request.path
            if request.path.startswith(self._mount + "/")
            else root.rstrip("/") + request.path
        )
        if full.startswith(self._mount + "/"):
            path = full[len(self._mount) + 1 :].split("/")
        elif root and request.path.startswith("/"):
            path = request.path.strip("/").split("/")
        else:
            return await _failure(send, 404, "not_found", "Unknown chat route")
        if any(not part or not _SEGMENT.match(part) or part in (".", "..") for part in path):
            return await _failure(send, 404, "not_found", "Unknown chat route")
        if path == ["agents"] and request.method == "GET":
            return await self._list_agents(request, send)
        agent = None
        if len(path) == 3:
            agent = self._agents.get(path[0])
        elif len(self._order) == 1:
            agent = self._agents.get(self._order[0])
        service, method = path[-2:] if len(path) >= 2 else ("", "")
        if (
            agent is None
            or len(path) not in (2, 3)
            or service != CHAT_SERVICE
            or method not in CHAT_METHODS
        ):
            return await _failure(send, 404, "not_found", "Unknown chat route")
        if request.method != "POST":
            return await _failure(send, 405, "unimplemented", "Use ConnectRPC POST")
        if not _CONTENT_TYPE.match(request.headers.get("content-type", "")):
            return await _failure(send, 415, "invalid_argument", "Connect content type required")
        auth = await self._headers(request, agent[0].agent_id)
        if auth is None:
            return await _failure(send, 401, "unauthenticated", "Sign in to chat")
        async with self._http.stream(
            "POST", f"{agent[1]}/{service}/{method}", headers=auth, content=_body(receive)
        ) as response:
            if 300 <= response.status < 400:
                return await _failure(
                    send, 502, "unavailable", "Provider redirects are not supported"
                )
            outgoing = [(b"cache-control", b"no-store"), (b"x-accel-buffering", b"no")]
            for name in _FORWARDED_RESPONSE:
                value = response.headers.get(name)
                if value:
                    outgoing.append((name.encode(), value.encode("latin-1")))
            await send(
                {"type": "http.response.start", "status": response.status, "headers": outgoing}
            )
            async for chunk in response.content:
                await send({"type": "http.response.body", "body": bytes(chunk), "more_body": True})
            await send({"type": "http.response.body", "body": b"", "more_body": False})

    async def _list_agents(self, request: ProxyRequest, send) -> None:
        listed = []
        for agent_id in self._order:
            agent, base = self._agents[agent_id]
            auth = await self._headers(request, agent_id)
            if auth is None:
                continue
            auth["content-type"] = "application/json"
            auth["connect-protocol-version"] = "1"
            response = await self._http.post(
                f"{base}/{CHAT_SERVICE}/ListAgents", headers=auth, content=b"{}"
            )
            if response.status != 200:
                return await _failure(send, 502, "unavailable", "Chat provider is unavailable")
            data = json.loads(response.content or b"{}")
            for entry in data.get("agents", []):
                if entry.get("id") == agent_id:
                    listed.append({"id": entry["id"], "name": entry.get("name", "")})
        if not listed:
            return await _failure(send, 401, "unauthenticated", "Sign in to chat")
        await _json(send, 200, {"agents": listed})


def create_chat_proxy(
    *,
    resolve_identity: ResolveIdentity,
    agent_id: str | None = None,
    api_key: str | None = None,
    base_url: str | None = None,
    base_url_override: str | None = None,
    agents: Sequence[ChatAgentConfig] | None = None,
    mount_path: str = "/api/chat",
    http_client: pyqwest.Client | None = None,
    trust_forwarded_headers: bool = True,
) -> ChatProxy:
    """Single-agent (``agent_id``/``api_key``) or multi-agent (``agents``) proxy."""
    if agents is None:
        if not agent_id or not api_key:
            raise ValueError("Supply agent_id and api_key, or agents")
        agents = [ChatAgentConfig(agent_id, api_key, base_url, base_url_override)]
    return ChatProxy(
        agents,
        resolve_identity=resolve_identity,
        mount_path=mount_path,
        http_client=http_client,
        trust_forwarded_headers=trust_forwarded_headers,
    )


def _origin(request: ProxyRequest, trust_forwarded: bool) -> str:
    """The public origin as a browser sees it.

    Behind a TLS-terminating reverse proxy the ASGI scheme is ``http`` while browsers send
    ``Origin: https://...``, so ``X-Forwarded-Proto``/``X-Forwarded-Host`` are honored by
    default. This only decides same-origin acceptance: a non-browser caller can already send
    any ``Origin`` and carries no browser credentials, and a cross-site page cannot add these
    headers without a CORS preflight, which the proxy never answers. Deployments that expose
    the ASGI server directly can set ``trust_forwarded_headers=False``.
    """
    scheme = request.scope.get("scheme", "http")
    host = request.headers.get("host", "")
    if trust_forwarded:
        scheme = request.headers.get("x-forwarded-proto", "").split(",")[0].strip() or scheme
        host = request.headers.get("x-forwarded-host", "").split(",")[0].strip() or host
    return f"{scheme}://{host}"


async def _body(receive) -> AsyncIterator[bytes]:
    while True:
        message = await receive()
        if message["type"] == "http.disconnect":
            return
        chunk = message.get("body", b"")
        if chunk:
            yield chunk
        if not message.get("more_body", False):
            return


async def _json(send, status: int, value: Any) -> None:
    body = json.dumps(value).encode()
    await send(
        {
            "type": "http.response.start",
            "status": status,
            "headers": [(b"content-type", b"application/json"), (b"cache-control", b"no-store")],
        }
    )
    await send({"type": "http.response.body", "body": body, "more_body": False})


async def _failure(send, status: int, code: str, message: str) -> None:
    await _json(send, status, {"code": code, "message": message})
