"""Management and runtime clients: generated Connect clients sharing one bearer token."""

from __future__ import annotations

from typing import Any, TypeVar
from urllib.parse import quote

from connectrpc.request import RequestContext

from tilde._transport import http_client
from tilde.chat_proxy import identity_header
from tilde.management.v1.access_connect import AgentAccessServiceClient
from tilde.management.v1.agents_connect import AgentServiceClient as ManagementAgentServiceClient
from tilde.management.v1.api_keys_connect import ApiKeysServiceClient
from tilde.management.v1.connections_connect import ConnectionsServiceClient
from tilde.management.v1.deployments_connect import DeploymentServiceClient
from tilde.management.v1.identities_connect import IdentitiesServiceClient
from tilde.management.v1.inference_connect import InferenceServiceClient
from tilde.management.v1.logs_connect import LogsServiceClient
from tilde.management.v1.tilde_chat_connect import TildeChatProviderServiceClient
from tilde.management.v1.tracing_connect import TracingServiceClient
from tilde.provider.tilde.v1.chat_connect import ChatServiceClient as TildeChatServiceClient
from tilde.runtime.v1.agents_connect import AgentServiceClient as RuntimeAgentServiceClient
from tilde.runtime.v1.chat_connect import ChatServiceClient as RuntimeChatServiceClient

T = TypeVar("T")


class _Bearer:
    """Metadata interceptor adding the bearer token (and chat identity) to every RPC."""

    def __init__(self, token: str | None, identity: str | None = None) -> None:
        self._token = token
        self._identity = identity

    async def on_start(self, ctx: RequestContext) -> None:
        if self._token:
            ctx.request_headers["authorization"] = f"Bearer {self._token}"
        if self._identity is not None:
            ctx.request_headers["x-tilde-identity"] = identity_header(self._identity)

    async def on_end(self, token: Any, ctx: RequestContext, error: Exception | None, /) -> None:
        return None


def _clients(base_url: str, access_token: str | None, *services: type[T]) -> list[T]:
    """One shared HTTP/2 client per group."""
    base = base_url.rstrip("/")
    client = http_client()
    interceptors = [_Bearer(access_token)]
    return [
        service(base, http_client=client, interceptors=interceptors)  # type: ignore[call-arg]
        for service in services
    ]


class ManagementClient:
    """Requires a bearer token from the installation's OIDC login; Tilde has no API keys."""

    def __init__(self, base_url: str, access_token: str | None = None) -> None:
        (
            self.access,
            self.agents,
            self.api_keys,
            self.connections,
            self.deployments,
            self.identities,
            self.inference,
            self.logs,
            self.tilde_chat,
            self.traces,
        ) = _clients(
            base_url,
            access_token,
            AgentAccessServiceClient,
            ManagementAgentServiceClient,
            ApiKeysServiceClient,
            ConnectionsServiceClient,
            DeploymentServiceClient,
            IdentitiesServiceClient,
            InferenceServiceClient,
            LogsServiceClient,
            TildeChatProviderServiceClient,
            TracingServiceClient,
        )


class RuntimeClient:
    """Programmatic runtime access with an agent connect token."""

    def __init__(self, base_url: str, access_token: str | None = None) -> None:
        self.agents, self.chat = _clients(
            base_url, access_token, RuntimeAgentServiceClient, RuntimeChatServiceClient
        )


def create_tilde_chat_client(
    base_url: str,
    agent_id: str,
    *,
    api_key: str | None = None,
    identity: str | None = None,
    access_token: str | None = None,
    base_url_override: str | None = None,
) -> TildeChatServiceClient:
    """Server-side client for the built-in Tilde chat provider.

    Embedding uses ``api_key`` plus the ``identity`` of your authenticated application user;
    ``access_token`` is the privileged operator form. Chat no longer lives on management.
    """
    if (api_key is None) == (access_token is None) or (api_key is not None and identity is None):
        raise ValueError("Supply api_key with identity, or access_token")
    base = base_url_override or f"{base_url.rstrip('/')}/agents/{quote(agent_id, safe='')}"
    return TildeChatServiceClient(
        base.rstrip("/"),
        http_client=http_client(),
        interceptors=[_Bearer(api_key or access_token, identity)],
    )


def create_management_client(base_url: str, access_token: str | None = None) -> ManagementClient:
    return ManagementClient(base_url, access_token)


def create_runtime_client(base_url: str, access_token: str | None = None) -> RuntimeClient:
    return RuntimeClient(base_url, access_token)
