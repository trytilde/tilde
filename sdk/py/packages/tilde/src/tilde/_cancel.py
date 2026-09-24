"""Cooperative cancellation shared by the host and the invocation context."""

from __future__ import annotations

import asyncio
from collections.abc import Callable


class StopLoop(BaseException):
    """Raised by ``ctx.stop()``; frameworks must let it propagate like a cancellation."""


class InvocationCancelled(BaseException):
    """The invocation is no longer active (stop control, host shutdown, lost callbacks)."""


class Cancellation:
    """AbortSignal equivalent: aborts once, remembers why and notifies registered callbacks."""

    def __init__(self) -> None:
        self._event = asyncio.Event()
        self.reason: BaseException | None = None
        self._callbacks: list[Callable[[], None]] = []

    @property
    def aborted(self) -> bool:
        return self._event.is_set()

    def abort(self, reason: BaseException | None = None) -> None:
        if self._event.is_set():
            return
        self.reason = reason
        self._event.set()
        callbacks, self._callbacks = self._callbacks, []
        for callback in callbacks:
            callback()

    def on_abort(self, callback: Callable[[], None]) -> None:
        if self._event.is_set():
            callback()
        else:
            self._callbacks.append(callback)

    def check(self) -> None:
        """Raise when aborted, like ``AbortSignal.throwIfAborted``."""
        if self._event.is_set():
            raise self.reason if self.reason is not None else InvocationCancelled()

    async def wait(self) -> None:
        await self._event.wait()
