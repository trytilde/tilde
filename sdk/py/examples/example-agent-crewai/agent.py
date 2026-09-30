"""The CrewAI crew: YAML-configured agent, module-level LLM, per-invocation Tilde and bundled
tools and registry skills."""

import asyncio
import logging
import os
from pathlib import Path

from bundled_tools import TOOLS
from crewai import LLM, Agent
from crewai.project import CrewBase, agent
from crewai.tools import BaseTool

import tilde
from tilde import AgentContext
from tilde_crewai import convert_to_crewai_messages, inference_interceptor, with_tilde_tools

log = logging.getLogger("example-agent-crewai")
MAX_ITERATIONS = 8
TIMEOUT_SECONDS = 60

# The inference connection this agent was given; the gateway holds its provider key. Built once:
# each request resolves the invocation running it.
INFERENCE = tilde.inference(os.environ.get("TILDE_INFERENCE", "default"))
llm = LLM(
    model=f"openai/{os.environ.get('OPENAI_MODEL', 'gpt-4o-mini')}",
    base_url=INFERENCE.base_url,
    api_key=INFERENCE.api_key,
    interceptor=inference_interceptor(INFERENCE),
    max_retries=0,
    max_tokens=600,
)
# Shipped with the deployment by `tilde deploy`; CrewAI discovers the folders at runtime.
SKILLS = tilde.define_skills("skills")


@CrewBase
class ExampleCrew:
    """One instance per invocation: the Tilde tools and registry skills belong to it."""

    agents_config = "config/agents.yaml"
    tasks_config = "config/tasks.yaml"

    def __init__(self, tools: list[BaseTool], registry_skills: Path) -> None:
        self.tools = tools
        self.registry_skills = registry_skills

    @agent
    def responder(self) -> Agent:
        return Agent(
            config=self.agents_config["responder"],  # type: ignore[index]
            llm=llm,
            tools=self.tools,
            # Skills assigned in Tilde reach the next invocation without a redeploy.
            skills=[SKILLS.path, self.registry_skills],
            max_iter=MAX_ITERATIONS,
        )


async def respond(ctx: AgentContext) -> None:
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
        tools = await with_tilde_tools(ctx, TOOLS.tools, options=TOOLS.options)
        registry_skills = Path(await ctx.skills.directory())
        responder = ExampleCrew(tools, registry_skills).responder()
        await asyncio.wait_for(responder.kickoff_async(messages), timeout=TIMEOUT_SECONDS)
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
