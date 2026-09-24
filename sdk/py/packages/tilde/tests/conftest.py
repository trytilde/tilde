"""Shared fixtures for the core tests."""

from __future__ import annotations

from collections.abc import AsyncIterator

import pytest
from support import Served

from tilde._transport import close_http_client


@pytest.fixture
async def served() -> AsyncIterator[list[Served]]:
    """Collect servers to close after the test; pooled SDK connections close first."""
    servers: list[Served] = []
    yield servers
    await close_http_client()
    for server in servers:
        await server.close()
