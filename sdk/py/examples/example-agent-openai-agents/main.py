"""Standalone Tilde agent host for local development with the OpenAI Agents SDK."""

import logging
import os

from agent import respond
from agents import set_tracing_disabled

from tilde import run_connected_agent


def main() -> None:
    if not os.environ.get("TILDE_GATEWAY_URL") or not os.environ.get("TILDE_DEPLOYMENT_TOKEN"):
        raise RuntimeError("TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN are required")
    set_tracing_disabled(True)  # Keep local traces out of OpenAI's trace export.
    logging.basicConfig(level=logging.INFO)
    logging.getLogger("example-agent-openai-agents").info(
        "Example agent (OpenAI Agents SDK) is dialing in to Tilde"
    )
    run_connected_agent(run=respond)


if __name__ == "__main__":
    main()
