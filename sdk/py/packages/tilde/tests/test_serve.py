"""serve() picks the host from where the entry module runs."""

import pytest

import tilde


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


def test_inside_lambda_serve_is_the_lambda_handler(plain: pytest.MonkeyPatch) -> None:
    plain.setenv("AWS_LAMBDA_FUNCTION_NAME", "agent")
    handler = tilde.serve(run=run)
    # It runs the event as an invocation (reporting failures rather than raising) instead of
    # refusing to run.
    try:
        handler({"notAField": 1})
    except RuntimeError as error:
        assert "only runs in AWS Lambda" not in str(error)
