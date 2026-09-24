"""Visible responses are explicit provider tool calls; a text-only model result sends nothing."""

import asyncio
import logging

from agents import Agent, ModelSettings, Runner
from openai import APIStatusError

from tilde import AgentContext
from tilde_openai_agents import convert_to_openai_agents_messages, convert_to_openai_agents_tools

log = logging.getLogger("example-agent-openai-agents")

INSTRUCTIONS = (
    "You are Example Agent 1, a helpful local development assistant. Use the current channel "
    "tools to respond to the latest message, concisely and helpfully. Model text is private and "
    "is not delivered to the user. Choose the appropriate provider tool using its instructions "
    "and conversation references. Use the supplied attachments when answering."
)


async def respond(ctx: AgentContext, model_name: str) -> None:
    log.info("Agent invocation started")
    stage = "history"
    try:
        history = await ctx.message.history()
        log.debug("Loaded conversation history (%d items)", len(history.items))
        items = await convert_to_openai_agents_messages(history.items, context=ctx)
        stage = "inference"
        agent = Agent(
            name="example-agent-openai-agents",
            instructions=INSTRUCTIONS,
            model=model_name,
            model_settings=ModelSettings(max_tokens=600, store=False),
            tools=convert_to_openai_agents_tools(ctx.channel.current),
        )
        result = await asyncio.wait_for(Runner.run(agent, items, max_turns=8), timeout=60)
        log.info("Agent invocation completed (%d new items)", len(result.new_items))
    except Exception as error:
        # Upstream response bodies can contain sensitive details; expose only the HTTP status.
        status_code = error.status_code if isinstance(error, APIStatusError) else None
        log.error("Agent invocation failed at %s (status %s)", stage, status_code)
        status = f" ({status_code})" if status_code else ""
        source = "OpenAI inference" if stage == "inference" else "Conversation history"
        raise RuntimeError(f"{source} failed{status}") from None
