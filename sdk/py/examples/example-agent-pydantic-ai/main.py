"""Standalone host: dials in to Tilde and routes replies through Pydantic AI."""

from __future__ import annotations

import logging
import os

from agent import respond
from pydantic_ai.models.openai import OpenAIResponsesModel
from pydantic_ai.providers.openai import OpenAIProvider

from tilde import AgentContext, run_connected_agent

log = logging.getLogger("example-agent-pydantic-ai")


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
    model = OpenAIResponsesModel(
        os.environ.get("OPENAI_MODEL", "gpt-4o-mini"), provider=OpenAIProvider(api_key=api_key)
    )

    async def run(ctx: AgentContext) -> None:
        await respond(ctx, model)

    logging.basicConfig(level=logging.INFO)
    log.info("Example Agent (Pydantic AI) is dialing in to Tilde")
    run_connected_agent(run=run)


if __name__ == "__main__":
    main()
