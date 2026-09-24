"""Reply generation: LangChain ``create_agent`` over the current channel's tools."""

import asyncio
import logging

from langchain.agents import create_agent
from langchain_core.language_models import BaseChatModel

from tilde import AgentContext
from tilde_langchain import convert_to_langchain_messages, convert_to_langchain_tools

log = logging.getLogger("example-agent-langchain")

INSTRUCTIONS = (
    "You are Example Agent 1, a helpful local development assistant. Use the current channel "
    "tools to respond to the latest message, concisely and helpfully. Model text is private "
    "and is not delivered to the user. Choose the appropriate provider tool using its "
    "instructions and conversation references. Use the supplied attachments when answering."
)


async def respond(ctx: AgentContext, model: BaseChatModel) -> None:
    """Visible responses are explicit provider tool calls; a text-only result sends nothing."""
    log.info("Agent invocation started")
    stage = "history"
    try:
        history = await ctx.message.history()
        log.debug("Loaded conversation history (%d items)", len(history.items))
        messages = await convert_to_langchain_messages(history.items, context=ctx)
        stage = "inference"
        agent = create_agent(
            model,
            tools=convert_to_langchain_tools(ctx.channel.current),
            system_prompt=INSTRUCTIONS,
        )
        async with asyncio.timeout(60):
            result = await agent.ainvoke({"messages": messages}, config={"recursion_limit": 16})
        log.info("Agent invocation completed (%d messages)", len(result["messages"]))
    except Exception as error:
        # Upstream response bodies can contain sensitive details; expose only the HTTP status.
        status_code = getattr(error, "status_code", None)
        log.error("Agent invocation failed at stage %s (status %s)", stage, status_code)
        status = f" ({status_code})" if status_code else ""
        source = "OpenAI inference" if stage == "inference" else "Conversation history"
        raise RuntimeError(f"{source} failed{status}") from None
