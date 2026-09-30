"""Standalone host: dials in to Tilde and routes replies through Pydantic AI."""

from __future__ import annotations

import logging
import os

from agent import respond

from tilde import run_connected_agent

log = logging.getLogger("example-agent-pydantic-ai")


def main() -> None:
    if not os.environ.get("TILDE_GATEWAY_URL") or not os.environ.get("TILDE_DEPLOYMENT_TOKEN"):
        raise RuntimeError("TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN are required")
    logging.basicConfig(level=logging.INFO)
    log.info("Example Agent (Pydantic AI) is dialing in to Tilde")
    run_connected_agent(run=respond)


if __name__ == "__main__":
    main()
