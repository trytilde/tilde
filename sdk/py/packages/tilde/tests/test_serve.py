"""serve() picks the host from where the entry module runs."""

import asyncio
import os
import signal
import sys

import pytest
from fake_gateway import FakeGateway
from support import serve_app

import tilde
import tilde.host


async def run(_context: tilde.AgentContext) -> None:
    return None


@pytest.fixture
def plain(monkeypatch: pytest.MonkeyPatch) -> pytest.MonkeyPatch:
    for name in ("TILDE_DISCOVERY", "AWS_LAMBDA_FUNCTION_NAME", "TILDE_DEPLOYMENT_TOKEN"):
        monkeypatch.delenv(name, raising=False)
    return monkeypatch


def test_a_plain_import_and_discovery_start_nothing(plain: pytest.MonkeyPatch) -> None:
    with pytest.raises(RuntimeError, match="only runs in AWS Lambda"):
        tilde.serve(run=run)({})
    # tilde deploy reads the module: no host starts, even with a token.
    plain.setenv("TILDE_DISCOVERY", "1")
    plain.setenv("TILDE_DEPLOYMENT_TOKEN", "deployment-secret")
    with pytest.raises(RuntimeError, match="only runs in AWS Lambda"):
        tilde.serve(run=run)({})


@pytest.mark.parametrize("token", [None, "deployment-secret"])
def test_inside_lambda_serve_is_the_lambda_handler(
    plain: pytest.MonkeyPatch, token: str | None
) -> None:
    plain.setenv("AWS_LAMBDA_FUNCTION_NAME", "agent")
    if token:
        plain.setenv("TILDE_DEPLOYMENT_TOKEN", token)
    handler = object()
    plain.setattr(tilde.host, "create_lambda_handler", lambda **_options: handler)
    assert tilde.serve(run=run) is handler


async def test_inside_an_event_loop_the_blocking_host_refuses(plain: pytest.MonkeyPatch) -> None:
    plain.setenv("TILDE_DEPLOYMENT_TOKEN", "deployment-secret")
    with pytest.raises(RuntimeError, match="connect_agent"):
        tilde.serve(run=run)


async def test_a_dial_in_host_closes_and_exits_on_sigterm() -> None:
    gateway = FakeGateway()
    upstream = await serve_app(gateway.app())
    script = "import tilde\nasync def run(context):\n    pass\ntilde.serve(run=run)\n"
    child = await asyncio.create_subprocess_exec(
        sys.executable,
        "-c",
        script,
        env={
            **os.environ,
            "TILDE_GATEWAY_URL": upstream.url,
            "TILDE_DEPLOYMENT_TOKEN": "deployment-secret",
        },
    )
    try:
        for _ in range(200):
            if "Bearer deployment-secret" in gateway.authorizations:
                break
            await asyncio.sleep(0.05)
        assert "Bearer deployment-secret" in gateway.authorizations, "dialled in"
        child.send_signal(signal.SIGTERM)
        # End the server side of Watch too: the fake server (Hypercorn) can otherwise linger on
        # the half-closed HTTP/2 stream the closing host leaves.
        gateway.wakes.put_nowait(None)
        assert await asyncio.wait_for(child.wait(), 15) == 0
    finally:
        if child.returncode is None:
            child.kill()
        await upstream.close()
