"""Standalone host for the LangChain example agent (``python main.py``)."""

import logging
import os

from agent import respond

from tilde import run_connected_agent

log = logging.getLogger("example-agent-langchain")


def main() -> None:
    if not os.environ.get("TILDE_GATEWAY_URL") or not os.environ.get("TILDE_DEPLOYMENT_TOKEN"):
        raise RuntimeError("TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN are required")
    log.info("Example LangChain agent is dialing in to Tilde")
    run_connected_agent(run=respond)


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    main()
