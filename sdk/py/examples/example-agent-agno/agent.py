"""Reply generation: Tilde history -> Agno messages, channel tools -> Agno functions."""

import asyncio
import logging

from agno.agent import Agent
from agno.models.base import Model

from tilde import AgentContext
from tilde_agno import convert_to_agno_messages, convert_to_agno_tools

log = logging.getLogger("example-agent-agno")

INSTRUCTIONS = (
    "You are Example Agent 1, a helpful local development assistant. Use the current channel "
    "tools to respond to the latest message, concisely and helpfully. Model text is private and "
    "is not delivered to the user. Choose the appropriate provider tool using its instructions "
    "and conversation references. Use the supplied attachments when answering."
)
TOOL_CALL_LIMIT = 8
TIMEOUT_SECONDS = 60


async def respond(ctx: AgentContext, model: Model) -> None:
    """Visible responses are explicit provider tool calls; a text-only result sends nothing."""
    log.info("Agent invocation started")
    stage = "history"
    try:
        history = await ctx.message.history()
        log.debug("Loaded conversation history: %d items", len(history.items))
        messages = await convert_to_agno_messages(history.items, context=ctx)
        if not messages:
            log.info("No context to respond to")
            return
        stage = "inference"
        agent = Agent(
            model=model,
            instructions=INSTRUCTIONS,
            tools=convert_to_agno_tools(ctx.channel.current),
            tool_call_limit=TOOL_CALL_LIMIT,
            telemetry=False,
        )
        result = await asyncio.wait_for(agent.arun(input=messages), timeout=TIMEOUT_SECONDS)
        log.info("Agent invocation completed: %d tool calls", len(result.tools or []))
    except Exception as error:
        if ctx.cancelled:
            log.warning("Agent invocation cancelled")
            raise
        # Upstream response bodies can contain sensitive details; expose only the status code.
        status = getattr(error, "status_code", None)
        log.error("Agent invocation failed at stage %s (status %s)", stage, status)
        label = "OpenAI inference" if stage == "inference" else "Conversation history"
        raise RuntimeError(f"{label} failed{f' ({status})' if status else ''}") from None
