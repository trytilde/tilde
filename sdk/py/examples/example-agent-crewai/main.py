"""Standalone Tilde host for the CrewAI example agent."""

import logging
import os

# CrewAI reads this when it is imported; it sends anonymous telemetry otherwise.
os.environ.setdefault("CREWAI_DISABLE_TELEMETRY", "true")

from agent import respond  # noqa: E402
from crewai import LLM  # noqa: E402

from tilde import run_connected_agent  # noqa: E402

log = logging.getLogger("example-agent-crewai")


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
    llm = LLM(
        model=f"openai/{os.environ.get('OPENAI_MODEL', 'gpt-4o-mini')}",
        api_key=api_key,
        max_retries=0,
        max_tokens=600,
    )
    log.info("Example agent (CrewAI) is dialing in to Tilde")
    run_connected_agent(run=lambda ctx: respond(ctx, llm))


if __name__ == "__main__":
    main()
