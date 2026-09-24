"""Standalone Tilde host for the Agno example agent."""

import logging
import os

from agent import respond
from agno.models.openai import OpenAIChat

from tilde import run_connected_agent

log = logging.getLogger("example-agent-agno")


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
    logging.basicConfig(level=logging.INFO)
    model = OpenAIChat(
        id=os.environ.get("OPENAI_MODEL", "gpt-4o-mini"),
        api_key=api_key,
        store=False,
        max_retries=0,
        max_tokens=600,
    )
    log.info("Example agent (Agno) is dialing in to Tilde")
    run_connected_agent(run=lambda ctx: respond(ctx, model))


if __name__ == "__main__":
    main()
