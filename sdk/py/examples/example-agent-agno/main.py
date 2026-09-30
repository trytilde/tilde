"""Standalone Tilde host for the Agno example agent."""

import logging
import os

from agent import respond

from tilde import run_connected_agent

log = logging.getLogger("example-agent-agno")


def main() -> None:
    if not os.environ.get("TILDE_GATEWAY_URL") or not os.environ.get("TILDE_DEPLOYMENT_TOKEN"):
        raise RuntimeError("TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN are required")
    logging.basicConfig(level=logging.INFO)
    log.info("Example agent (Agno) is dialing in to Tilde")
    run_connected_agent(run=respond)


if __name__ == "__main__":
    main()
