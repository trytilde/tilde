"""Standalone Tilde host for the CrewAI example agent."""

import logging
import os

# CrewAI reads this when it is imported; it sends anonymous telemetry otherwise.
os.environ.setdefault("CREWAI_DISABLE_TELEMETRY", "true")

from agent import respond  # noqa: E402

from tilde import run_connected_agent  # noqa: E402

log = logging.getLogger("example-agent-crewai")


def main() -> None:
    if not os.environ.get("TILDE_GATEWAY_URL") or not os.environ.get("TILDE_DEPLOYMENT_TOKEN"):
        raise RuntimeError("TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN are required")
    logging.basicConfig(level=logging.INFO)
    log.info("Example agent (CrewAI) is dialing in to Tilde")
    run_connected_agent(run=respond)


if __name__ == "__main__":
    main()
