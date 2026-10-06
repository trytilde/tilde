"""The LangChain agent, created once at module scope. ``tilde_middleware(bundled=TOOLS)``
gives each invocation the current channel's tools, its Tilde and bundled tools, steering and
skills."""

import asyncio
import logging
import os

from bundled_tools import TOOL_GUIDANCE, TOOLS
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI

import tilde
from tilde import AgentContext
from tilde_langchain import convert_to_langchain_messages, tilde_middleware

log = logging.getLogger("example-agent-langchain")

# The inference connection this agent was given; the gateway holds its provider key. Built once:
# each request resolves the invocation running it.
INFERENCE = tilde.inference(os.environ.get("TILDE_INFERENCE", "default"))

agent = create_agent(
    ChatOpenAI(
        model=os.environ.get("OPENAI_MODEL", "gpt-5.6-terra"),
        # GPT-5.6 takes function tools with reasoning only on the Responses API.
        use_responses_api=True,
        base_url=INFERENCE.base_url,
        api_key=INFERENCE.api_key,
        http_async_client=INFERENCE.async_client(),
        http_client=INFERENCE.client(),
        max_retries=0,
        timeout=60,
    ),
    system_prompt=(
        "You are Example Agent 1, a helpful local development assistant. Use the current channel "
        "tools to respond to the latest message, concisely and helpfully. Model text is private "
        "and is not delivered to the user. Choose the appropriate provider tool using its "
        "instructions and conversation references. Use the supplied attachments when answering."
        + TOOL_GUIDANCE
    ),
    middleware=[tilde_middleware(bundled=TOOLS)],
    name="example-agent-langchain",
)


async def respond(ctx: AgentContext) -> None:
    """Visible responses are explicit provider tool calls; a text-only result sends nothing."""
    log.info("Agent invocation started")
    stage = "history"
    try:
        history = await ctx.message.history()
        log.debug("Loaded conversation history (%d items)", len(history.items))
        messages = await convert_to_langchain_messages(history.items, context=ctx)
        stage = "inference"
        async with asyncio.timeout(60):
            result = await agent.ainvoke({"messages": messages}, {"recursion_limit": 16})
        log.info("Agent invocation completed (%d messages)", len(result["messages"]))
    except Exception as error:
        if ctx.cancelled:
            log.warning("Agent invocation cancelled")
            raise
        # Upstream response bodies can contain sensitive details; expose only the HTTP status.
        status_code = getattr(error, "status_code", None)
        log.error("Agent invocation failed at stage %s (status %s)", stage, status_code)
        status = f" ({status_code})" if status_code else ""
        source = "OpenAI inference" if stage == "inference" else "Conversation history"
        raise RuntimeError(f"{source} failed{status}") from None
