"""The Agno agent, defined once at module scope; each invocation runs it with Tilde's per-run
pieces (channel, Tilde and bundled tools, steering, registry skills, cancellation). Model text
stays private."""

import asyncio
import logging
import os
from pathlib import Path

from agno.agent import Agent
from agno.models.openai import OpenAIChat
from agno.skills import LocalSkills
from bundled_tools import TOOL_GUIDANCE, TOOLS

import tilde
from tilde import AgentContext
from tilde_agno import TildeSkills, convert_to_agno_messages, tilde_agno

log = logging.getLogger("example-agent-agno")
TIMEOUT_SECONDS = 60

# The inference connection this agent was given; the gateway holds its provider key. Built once:
# each request resolves the invocation running it.
INFERENCE = tilde.inference(os.environ.get("TILDE_INFERENCE", "default"))
responder = Agent(
    model=OpenAIChat(
        id=os.environ.get("OPENAI_MODEL", "gpt-4o-mini"),
        base_url=INFERENCE.base_url,
        api_key=INFERENCE.api_key,
        http_client=INFERENCE.async_client(),
        store=False,
        max_retries=0,
        max_tokens=600,
    ),
    instructions=(
        "You are Example Agent 1, a helpful local development assistant. Use the current channel "
        "tools to respond to the latest message, concisely and helpfully. Model text is private "
        "and is not delivered to the user. Choose the appropriate provider tool using its "
        "instructions and conversation references. Use the supplied attachments when answering."
        + TOOL_GUIDANCE
    ),
    # Shipped with the deployment by `tilde deploy`; skills assigned in Tilde join per invocation.
    skills=TildeSkills([LocalSkills(str(Path(__file__).parent / "skills"))]),
    tool_call_limit=8,
    telemetry=False,
)


async def respond(ctx: AgentContext) -> None:
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
        result = await asyncio.wait_for(
            responder.arun(input=messages, **await tilde_agno(ctx, responder, bundled=TOOLS)),
            timeout=TIMEOUT_SECONDS,
        )
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
