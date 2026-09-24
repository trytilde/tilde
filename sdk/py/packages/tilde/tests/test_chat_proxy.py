"""Chat proxy across the ASGI boundary: routing, identity injection, streaming and denial."""

from __future__ import annotations

import base64
import json

import pyqwest
from support import serve_app

from tilde.chat_proxy import CHAT_SERVICE, ChatAgentConfig, create_chat_proxy


class Upstream:
    """Records what reaches the provider and streams a framed reply back."""

    def __init__(self) -> None:
        self.requests: list[dict] = []

    async def __call__(self, scope, receive, send) -> None:
        if scope["type"] == "lifespan":
            while True:
                message = await receive()
                if message["type"] == "lifespan.startup":
                    await send({"type": "lifespan.startup.complete"})
                elif message["type"] == "lifespan.shutdown":
                    await send({"type": "lifespan.shutdown.complete"})
                    return
        body = b""
        while True:
            message = await receive()
            body += message.get("body", b"")
            if not message.get("more_body"):
                break
        headers = {k.decode(): v.decode() for k, v in scope["headers"]}
        self.requests.append({"path": scope["path"], "headers": headers, "body": body})
        if scope["path"].endswith("/ListAgents"):
            await send(
                {
                    "type": "http.response.start",
                    "status": 200,
                    "headers": [(b"content-type", b"application/json")],
                }
            )
            await send(
                {
                    "type": "http.response.body",
                    "body": json.dumps({"agents": [{"id": "agent-1", "name": "Helper"}]}).encode(),
                }
            )
            return
        await send(
            {
                "type": "http.response.start",
                "status": 200,
                "headers": [(b"content-type", b"application/connect+json"), (b"x-secret", b"1")],
            }
        )
        for chunk in (b"frame-1", b"frame-2"):
            await send({"type": "http.response.body", "body": chunk, "more_body": True})
        await send({"type": "http.response.body", "body": b"", "more_body": False})


async def test_proxy_forwards_allowed_methods_with_server_identity(served):
    upstream = Upstream()
    provider = await serve_app(upstream)
    served.append(provider)
    seen: list[tuple[str, str]] = []

    async def resolve_identity(request, agent_id):
        seen.append((request.headers.get("cookie", ""), agent_id))
        return "user-42" if request.headers.get("cookie") == "session=ok" else None

    proxy = create_chat_proxy(
        agents=[ChatAgentConfig("agent-1", "tilde_chat_secret", base_url_override=provider.url)],
        resolve_identity=resolve_identity,
    )
    host = await serve_app(proxy)
    served.append(host)
    http = pyqwest.Client(pyqwest.HTTPTransport(follow_redirects=False))
    base = f"{host.url}/api/chat"

    denied = await http.post(
        f"{base}/{CHAT_SERVICE}/ListSessions",
        headers={"content-type": "application/json"},
        content=b"{}",
    )
    assert denied.status == 401 and upstream.requests == []

    allowed = await http.post(
        f"{base}/{CHAT_SERVICE}/WatchThread",
        headers={
            "content-type": "application/connect+json",
            "cookie": "session=ok",
            "authorization": "Bearer browser-token-must-not-pass",
            "x-tilde-identity": "forged",
            "connect-protocol-version": "1",
        },
        content=b"stream-body",
    )
    assert allowed.status == 200
    assert allowed.content == b"frame-1frame-2"
    assert allowed.headers.get("content-type") == "application/connect+json"
    assert allowed.headers.get("x-secret") is None
    assert allowed.headers.get("cache-control") == "no-store"
    forwarded = upstream.requests[-1]
    assert forwarded["path"] == f"/{CHAT_SERVICE}/WatchThread"
    assert forwarded["headers"]["authorization"] == "Bearer tilde_chat_secret"
    assert forwarded["headers"]["x-tilde-identity"] == base64.urlsafe_b64encode(
        b"user-42"
    ).decode().rstrip("=")
    assert "cookie" not in forwarded["headers"]
    assert forwarded["body"] == b"stream-body"

    unknown = await http.post(
        f"{base}/{CHAT_SERVICE}/DeleteEverything",
        headers={"content-type": "application/json", "cookie": "session=ok"},
        content=b"{}",
    )
    assert unknown.status == 404

    cross_origin = await http.post(
        f"{base}/{CHAT_SERVICE}/ListSessions",
        headers={
            "content-type": "application/json",
            "cookie": "session=ok",
            "origin": "https://evil.example",
        },
        content=b"{}",
    )
    assert cross_origin.status == 403

    behind_tls = await http.post(
        f"{base}/{CHAT_SERVICE}/ListSessions",
        headers={
            "content-type": "application/json",
            "cookie": "session=ok",
            "origin": "https://chat.example",
            "x-forwarded-proto": "https",
            "x-forwarded-host": "chat.example",
        },
        content=b"{}",
    )
    assert behind_tls.status == 200
    strict = create_chat_proxy(
        agents=[ChatAgentConfig("agent-1", "tilde_chat_secret", base_url_override=provider.url)],
        resolve_identity=resolve_identity,
        trust_forwarded_headers=False,
    )
    strict_host = await serve_app(strict)
    served.append(strict_host)
    ignored = await http.post(
        f"{strict_host.url}/api/chat/{CHAT_SERVICE}/ListSessions",
        headers={
            "content-type": "application/json",
            "cookie": "session=ok",
            "origin": "https://chat.example",
            "x-forwarded-proto": "https",
            "x-forwarded-host": "chat.example",
        },
        content=b"{}",
    )
    assert ignored.status == 403

    agents = await http.get(f"{base}/agents", headers={"cookie": "session=ok"})
    assert agents.status == 200 and json.loads(agents.content) == {
        "agents": [{"id": "agent-1", "name": "Helper"}]
    }
    assert seen[0] == ("", "agent-1") and seen[1] == ("session=ok", "agent-1")
