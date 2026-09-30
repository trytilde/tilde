"""Route model calls through Tilde's inference gateway with the invocation's own credentials.

``ctx.inference("openai/prod")`` (or an alias the agent was given, such as ``"default"``)
returns what an OpenAI-compatible client needs: ``base_url``, a placeholder ``api_key`` and an
``httpx`` client whose auth hook sends the current, renewed invocation token on every request.
The gateway swaps in the real provider credential and accounts the call, so agents never hold
provider keys.

``tilde.inference(alias)`` is the module-level form for model clients built at import time:
every request resolves the invocation running it (token, callback URL, prompt stamps), and a
request made outside an invocation fails.
"""

from __future__ import annotations

import re
from collections.abc import Callable, Generator
from typing import Any

import httpx

from tilde._invocation import require_invocation

_SLUG = re.compile(r"^([a-z0-9_]+/)?[A-Za-z0-9._@+:-]+$")
# Module-level clients are built before any invocation exists; each request is rebased onto
# the invocation's callback URL, so this host is never dialed.
_UNBOUND = "http://tilde.invalid"


class _InvocationAuth(httpx.Auth):
    def __init__(self, prepare: Callable[[httpx.Request], httpx.Request]) -> None:
        self._prepare = prepare

    def auth_flow(self, request: httpx.Request) -> Generator[httpx.Request, httpx.Response, None]:
        yield self._prepare(request)


class Inference:
    """Connection details for one inference slug."""

    api_key = "tilde"

    def __init__(
        self,
        base_url: str,
        authorization: Callable[[], str],
        prompts: Callable[[], str] = lambda: "",
        callback_url: Callable[[], str] | None = None,
    ) -> None:
        self.base_url = base_url
        self._authorization = authorization
        self._prompts = prompts
        self._callback_url = callback_url
        self.auth: httpx.Auth = _InvocationAuth(self.prepare)

    def prepare(self, request: httpx.Request) -> httpx.Request:
        """Stamp one outgoing request, for frameworks that hook requests instead of clients."""
        if self._callback_url is not None:
            base = httpx.URL(self._callback_url().rstrip("/"))
            request.url = request.url.copy_with(
                scheme=base.scheme,
                host=base.host,
                port=base.port,
                raw_path=base.raw_path.rstrip(b"/") + request.url.raw_path,
            )
            request.headers["host"] = request.url.netloc.decode("ascii")
        # Replaces whatever key the provider SDK set; the gateway authenticates this token.
        request.headers["authorization"] = self._authorization()
        stamps = self._prompts()
        if stamps:
            request.headers["x-tilde-prompt"] = stamps
        else:
            request.headers.pop("x-tilde-prompt", None)
        return request

    def async_client(self, **options: Any) -> httpx.AsyncClient:
        """An ``httpx.AsyncClient`` for async provider SDKs (``AsyncOpenAI(http_client=...)``)."""
        return httpx.AsyncClient(auth=self.auth, **{"timeout": 600.0, **options})

    def client(self, **options: Any) -> httpx.Client:
        """A synchronous ``httpx.Client`` for provider SDKs that also call synchronously."""
        return httpx.Client(auth=self.auth, **{"timeout": 600.0, **options})


def _check(slug: str) -> None:
    if not _SLUG.match(slug):
        raise ValueError("Inference slug must be provider/account or an alias")


def bound_inference(
    callback_url: str, slug: str, authorization: Callable[[], str], prompts: Callable[[], str]
) -> Inference:
    _check(slug)
    return Inference(f"{callback_url.rstrip('/')}/inference/{slug}", authorization, prompts)


def inference(slug: str) -> Inference:
    """Inference for the invocation running each request; safe to build at import time."""
    _check(slug)
    what = f"tilde.inference({slug!r})"
    return Inference(
        f"{_UNBOUND}/inference/{slug}",
        lambda: require_invocation(what).trace_authorization(),
        lambda: require_invocation(what).prompt_stamps(),
        lambda: require_invocation(what).callback_url,
    )
