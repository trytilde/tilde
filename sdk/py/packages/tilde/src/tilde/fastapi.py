"""FastAPI/Starlette integration: a lifespan that hosts a dial-in agent, and the chat proxy.

Tilde never calls the application for agent work; ``agent_lifespan`` dials out, so the app
exposes no agent route.
"""

from __future__ import annotations

from collections.abc import AsyncIterator, Awaitable, Callable, Sequence
from contextlib import AbstractAsyncContextManager, asynccontextmanager
from typing import Any

from starlette.requests import Request

from tilde.chat_proxy import ChatAgentConfig, ChatProxy, ProxyRequest, create_chat_proxy
from tilde.host import connect_agent


def agent_lifespan(**options: Any) -> Callable[[Any], AbstractAsyncContextManager[None]]:
    """``FastAPI(lifespan=...)``: ``connect_agent(**options)`` on startup, ``close()`` on exit."""

    @asynccontextmanager
    async def lifespan(_app: Any) -> AsyncIterator[None]:
        connected = connect_agent(**options)
        try:
            yield
        finally:
            await connected.close()

    return lifespan


def mount_chat_proxy(
    app: Any,
    path: str = "/api/chat",
    *,
    resolve_identity: Callable[[Request, str], str | None | Awaitable[str | None]],
    agent_id: str | None = None,
    api_key: str | None = None,
    base_url: str | None = None,
    base_url_override: str | None = None,
    agents: Sequence[ChatAgentConfig] | None = None,
    trust_forwarded_headers: bool = True,
) -> ChatProxy:
    """Mount the chat proxy; ``resolve_identity(request, agent_id)`` receives the Starlette request.

    Derive the identity from verified server authentication (a session cookie or bearer token),
    never from a browser-supplied identity field. Return None to deny access to that agent.
    Keep the API key on the server; it never reaches the browser.
    """

    async def resolve(request: ProxyRequest, agent: str) -> str | None:
        identity = resolve_identity(Request(dict(request.scope)), agent)
        if hasattr(identity, "__await__"):
            identity = await identity  # type: ignore[misc]
        return identity  # type: ignore[return-value]

    proxy = create_chat_proxy(
        resolve_identity=resolve,
        agent_id=agent_id,
        api_key=api_key,
        base_url=base_url,
        base_url_override=base_url_override,
        agents=agents,
        mount_path=path.rstrip("/"),
        trust_forwarded_headers=trust_forwarded_headers,
    )
    app.mount(path.rstrip("/"), proxy)
    return proxy
