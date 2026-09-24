"""One HTTP/2 transport per process for callbacks, matching the Node SDK's HTTP/2 clients."""

from __future__ import annotations

import pyqwest

_transport: pyqwest.HTTPTransport | None = None


def http_client() -> pyqwest.Client:
    global _transport
    if _transport is None:
        # Plaintext callback URLs use HTTP/2 prior knowledge; TLS negotiates HTTP/2 through ALPN.
        # Redirects are refused so bearer credentials never follow an upstream redirect.
        _transport = pyqwest.HTTPTransport(
            http_version=pyqwest.HTTPVersion.HTTP2,
            follow_redirects=False,
            tls_include_system_certs=True,
        )
    return pyqwest.Client(_transport)


async def close_http_client() -> None:
    """Close pooled callback connections, for example before a test server shuts down."""
    global _transport
    transport, _transport = _transport, None
    if transport is not None:
        await transport.aclose()
