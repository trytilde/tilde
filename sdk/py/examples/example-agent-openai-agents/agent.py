"""The agent, defined once at module scope; each invocation runs it with the channel's
tools and the agent's Tilde and bundled tools.

Visible responses are explicit provider tool calls; a text-only model result sends nothing."""

import asyncio
import logging
import os

from agents import Agent, ModelSettings, OpenAIResponsesModel, Runner
from bundled_tools import TOOL_GUIDANCE, TOOLS
from openai import APIStatusError, AsyncOpenAI

import tilde
from tilde import AgentContext
from tilde_openai_agents import convert_to_openai_agents_messages, tilde_openai_agents

log = logging.getLogger("example-agent-openai-agents")

# The inference connection this agent was given; the gateway holds its provider key. Built once:
# each request resolves the invocation running it.
INFERENCE = tilde.inference(os.environ.get("TILDE_INFERENCE", "default"))
client = AsyncOpenAI(
    base_url=INFERENCE.base_url,
    api_key=INFERENCE.api_key,
    http_client=INFERENCE.async_client(),
    max_retries=0,
)

agent = Agent(
    name="example-agent-openai-agents",
    instructions=(
        "You are Example Agent 1, a helpful local development assistant. Use the current channel "
        "tools to respond to the latest message, concisely and helpfully. Model text is private "
        "and is not delivered to the user. Choose the appropriate provider tool using its "
        "instructions and conversation references. Use the supplied attachments when answering."
        + TOOL_GUIDANCE
    ),
    model=OpenAIResponsesModel(
        model=os.environ.get("OPENAI_MODEL", "gpt-5.6-terra"), openai_client=client
    ),
    model_settings=ModelSettings(store=False),
)


async def respond(ctx: AgentContext) -> None:
    log.info("Agent invocation started")
    stage = "history"
    try:
        history = await ctx.message.history()
        log.debug("Loaded conversation history (%d items)", len(history.items))
        items = await convert_to_openai_agents_messages(history.items, context=ctx)
        stage = "inference"
        run_agent, run_config = await tilde_openai_agents(ctx, agent, bundled=TOOLS)
        result = await asyncio.wait_for(
            Runner.run(run_agent, items, run_config=run_config, max_turns=8), timeout=60
        )
        log.info("Agent invocation completed (%d new items)", len(result.new_items))
    except Exception as error:
        if ctx.cancelled:
            log.warning("Agent invocation cancelled")
            raise
        # Upstream response bodies can contain sensitive details; expose only the HTTP status.
        status_code = error.status_code if isinstance(error, APIStatusError) else None
        log.error("Agent invocation failed at %s (status %s)", stage, status_code)
        status = f" ({status_code})" if status_code else ""
        source = "OpenAI inference" if stage == "inference" else "Conversation history"
        raise RuntimeError(f"{source} failed{status}") from None
