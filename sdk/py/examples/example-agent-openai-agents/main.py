"""Standalone Tilde agent host for local development with the OpenAI Agents SDK."""

import logging
import os

from agent import respond
from agents import set_default_openai_key, set_tracing_disabled

from tilde import run_connected_agent


def main() -> None:
    api_key = os.environ.get("OPENAI_API_KEY")
    if (
        not api_key
        or not os.environ.get("TILDE_GATEWAY_URL")
        or not os.environ.get("TILDE_DEPLOYMENT_TOKEN")
    ):
        raise RuntimeError(
            "OPENAI_API_KEY, TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN are required"
        )
    model_name = os.environ.get("OPENAI_MODEL", "gpt-4o-mini")
    set_default_openai_key(api_key)
    set_tracing_disabled(True)  # Keep local traces out of OpenAI's trace export.
    logging.basicConfig(level=logging.INFO)
    logging.getLogger("example-agent-openai-agents").info(
        "Example agent (OpenAI Agents SDK) is dialing in to Tilde"
    )
    run_connected_agent(run=lambda ctx: respond(ctx, model_name))


if __name__ == "__main__":
    main()
