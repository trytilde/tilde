"""Minimal OTLP/HTTP protobuf uploads to the invocation callback with the current connect token."""

from __future__ import annotations

import logging
import urllib.error
import urllib.request
from collections.abc import Callable
from urllib.parse import urlsplit, urlunsplit

log = logging.getLogger("tilde")


def otlp_url(base_url: str, signal: str) -> str:
    """Match the runtime RPC base path when a reverse proxy separates audiences by prefix."""
    parts = urlsplit(base_url)
    path = f"{parts.path.rstrip('/')}/v1/{signal}"
    return urlunsplit((parts.scheme, parts.netloc, path, "", ""))


def post_protobuf(
    url: str, payload: bytes, authorization: Callable[[], str], timeout: float = 15.0
) -> bool:
    request = urllib.request.Request(
        url,
        data=payload,
        method="POST",
        headers={"Content-Type": "application/x-protobuf", "Authorization": authorization()},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:  # noqa: S310 - caller-supplied callback URL
            return 200 <= response.status < 300
    except (urllib.error.URLError, OSError, ValueError) as error:
        log.debug("OTLP upload to %s failed: %s", url, error)
        return False
