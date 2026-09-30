"""The invocation the current task is running, for module-level objects built at import time."""

from __future__ import annotations

from contextvars import ContextVar
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from tilde.context import AgentContext

# Set by the host around ``run(ctx)``; asyncio tasks and ``asyncio.to_thread`` inherit it.
current: ContextVar[AgentContext | None] = ContextVar("tilde_invocation", default=None)


def current_invocation() -> AgentContext | None:
    """The running invocation's context, or ``None`` outside ``run(ctx)``."""
    return current.get()


def require_invocation(what: str) -> AgentContext:
    ctx = current.get()
    if ctx is None:
        raise RuntimeError(
            f"{what} was used outside a Tilde invocation; call it from within run(ctx)"
        )
    return ctx
