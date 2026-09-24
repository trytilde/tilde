"""Serve ASGI apps with Hypercorn (HTTP/1.1 and h2c) on free loopback ports for tests."""

from __future__ import annotations

import asyncio
import socket

from hypercorn.asyncio import serve
from hypercorn.config import Config


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Served:
    def __init__(self, url: str, task: asyncio.Task[None], stop: asyncio.Event) -> None:
        self.url = url
        self._task = task
        self._stop = stop

    async def close(self) -> None:
        self._stop.set()
        await asyncio.wait({self._task}, timeout=10)
        if not self._task.done():
            # Hypercorn can linger on half-closed HTTP/2 streams; never block teardown on it.
            self._task.cancel()
            await asyncio.wait({self._task}, timeout=5)


async def serve_app(app) -> Served:
    port = free_port()
    config = Config()
    config.bind = [f"127.0.0.1:{port}"]
    config.accesslog = None
    config.errorlog = None
    config.graceful_timeout = 1
    stop = asyncio.Event()

    async def trigger() -> None:
        await stop.wait()

    task = asyncio.create_task(serve(app, config, shutdown_trigger=trigger))
    for _ in range(100):
        await asyncio.sleep(0.05)
        try:
            reader, writer = await asyncio.open_connection("127.0.0.1", port)
            writer.close()
            await writer.wait_closed()
            break
        except OSError:
            continue
    return Served(f"http://127.0.0.1:{port}", task, stop)
