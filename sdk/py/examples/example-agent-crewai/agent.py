"""Reply generation: Tilde history -> CrewAI messages, channel tools -> CrewAI tools."""

import asyncio
import logging

from crewai import Agent
from crewai.llms.base_llm import BaseLLM

from tilde import AgentContext
from tilde_crewai import convert_to_crewai_messages, convert_to_crewai_tools

log = logging.getLogger("example-agent-crewai")

ROLE = "Example Agent 1, a helpful local development assistant"
GOAL = "Use the current channel tools to respond to the latest message, concisely and helpfully."
BACKSTORY = (
    "Model text is private and is not delivered to the user. Choose the appropriate provider "
    "tool using its instructions and conversation references. Use the supplied attachments "
    "when answering."
)
MAX_ITERATIONS = 8
TIMEOUT_SECONDS = 60


async def respond(ctx: AgentContext, llm: BaseLLM) -> None:
    """Visible responses are explicit provider tool calls; a text-only result sends nothing."""
    log.info("Agent invocation started")
    stage = "history"
    try:
        history = await ctx.message.history()
        log.debug("Loaded conversation history: %d items", len(history.items))
        messages = await convert_to_crewai_messages(history.items, context=ctx)
        if not messages:
            log.info("No context to respond to")
            return
        stage = "inference"
        agent = Agent(
            role=ROLE,
            goal=GOAL,
            backstory=BACKSTORY,
            llm=llm,
            tools=convert_to_crewai_tools(ctx.channel.current),
            max_iter=MAX_ITERATIONS,
        )
        await asyncio.wait_for(agent.kickoff_async(messages), timeout=TIMEOUT_SECONDS)
        log.info("Agent invocation completed")
    except Exception as error:
        if ctx.cancelled:
            log.warning("Agent invocation cancelled")
            raise
        # Upstream response bodies can contain sensitive details; expose only the status code.
        status = getattr(error, "status_code", None)
        log.error("Agent invocation failed at stage %s (status %s)", stage, status)
        label = "OpenAI inference" if stage == "inference" else "Conversation history"
        raise RuntimeError(f"{label} failed{f' ({status})' if status else ''}") from None
