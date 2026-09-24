"""Reply generation: history in, provider tool calls out. Model text stays private."""

from __future__ import annotations

import asyncio
import logging

from pydantic_ai import Agent
from pydantic_ai.exceptions import ModelHTTPError
from pydantic_ai.models import Model
from pydantic_ai.models.openai import OpenAIResponsesModelSettings
from pydantic_ai.usage import UsageLimits

from tilde import AgentContext, InvocationCancelled, StopLoop
from tilde_pydantic_ai import convert_to_pydantic_ai_messages, convert_to_pydantic_ai_tools

log = logging.getLogger("example-agent-pydantic-ai")

INSTRUCTIONS = (
    "You are Example Agent 1, a helpful local development assistant. Use the current channel "
    "tools to respond to the latest message, concisely and helpfully. Model text is private and "
    "is not delivered to the user. Choose the appropriate provider tool using its instructions "
    "and conversation references. Use the supplied attachments when answering."
)


async def respond(ctx: AgentContext, model: Model) -> None:
    """Visible responses are explicit provider tool calls; a text-only result sends nothing."""
    log.info("Agent invocation started")
    stage = "history"
    try:
        history = await ctx.message.history()
        log.debug("Loaded conversation history: %d items", len(history.items))
        messages = await convert_to_pydantic_ai_messages(history.items, context=ctx)
        stage = "inference"
        agent = Agent(
            model,
            instructions=INSTRUCTIONS,
            tools=convert_to_pydantic_ai_tools(ctx.channel.current),
            retries=0,
            model_settings=OpenAIResponsesModelSettings(max_tokens=600, openai_store=False),
        )
        async with asyncio.timeout(60):
            result = await agent.run(
                message_history=messages, usage_limits=UsageLimits(request_limit=8)
            )
        log.info("Agent invocation completed: %d requests", result.usage.requests)
    except (StopLoop, InvocationCancelled, asyncio.CancelledError):
        log.warning("Agent invocation cancelled")
        raise
    except Exception as error:
        # Upstream response bodies can contain sensitive details; expose only the HTTP status.
        status = error.status_code if isinstance(error, ModelHTTPError) else None
        log.error("Agent invocation failed at stage %s (status %s)", stage, status)
        suffix = f" ({status})" if status else ""
        source = "OpenAI inference" if stage == "inference" else "Conversation history"
        raise RuntimeError(f"{source} failed{suffix}") from None
