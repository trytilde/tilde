"""Standalone host for the LangChain example agent (``python main.py``)."""

import logging
import os

from agent import respond
from langchain_openai import ChatOpenAI

from tilde import run_connected_agent

log = logging.getLogger("example-agent-langchain")


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
    model = ChatOpenAI(
        model=os.environ.get("OPENAI_MODEL", "gpt-4o-mini"),
        api_key=api_key,
        max_tokens=600,
        max_retries=0,
        timeout=60,
    )
    log.info("Example LangChain agent is dialing in to Tilde")
    run_connected_agent(run=lambda ctx: respond(ctx, model))


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    main()
