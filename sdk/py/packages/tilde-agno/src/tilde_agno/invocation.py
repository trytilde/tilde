"""The per-invocation pieces of an Agno run, as ``agent.arun`` keyword arguments."""

from __future__ import annotations

import asyncio
import json
import sys
import uuid
from collections.abc import Awaitable, Callable
from contextvars import ContextVar
from typing import Any

from agno.agent import Agent
from agno.run import RunContext
from agno.run.cancel import cancel_run
from agno.skills import LocalSkills, Skill, Skills
from agno.team import Team

from tilde import AgentContext, BundledTools
from tilde.discovery import PROMPT_FORMAT_DYNAMIC
from tilde_agno.discover import agent_prompts
from tilde_agno.tools import convert_to_agno_tools, with_tilde_tools

_registry: ContextVar[dict[str, Skill] | None] = ContextVar("tilde_agno_skills", default=None)


class TildeSkills(Skills):
    """Agno ``Skills`` that also serve the skills assigned to the agent in Tilde.

    Use it in place of ``Skills``: ``Agent(skills=TildeSkills([LocalSkills("skills")]))``.
    ``tilde_agno`` loads the invocation's registry skills (assigned in Tilde, not shipped with
    the deployment) before each run, so a skill assigned in the UI reaches the next invocation;
    the agent's own skills win a name clash. Agno reads loaded skills from ``Skills._skills``
    (3.0.10), which this class resolves per invocation.
    """

    @property
    def _skills(self) -> dict[str, Skill]:
        own = self.__dict__["_own"]
        registry = _registry.get()
        return {**registry, **own} if registry else own

    @_skills.setter
    def _skills(self, value: dict[str, Skill]) -> None:
        self.__dict__["_own"] = value

    def reload(self) -> None:
        # Reload the agent's own skills, never an invocation's merged view.
        token = _registry.set(None)
        try:
            super().reload()
        finally:
            _registry.reset(token)


async def tilde_agno(
    ctx: AgentContext, agent: Agent | Team, *, bundled: BundledTools | None = None
) -> dict[str, Any]:
    """``await agent.arun(input, **await tilde_agno(ctx, agent, bundled=TOOLS))``.

    - ``run_context.client_tools``: the current channel's tools, the agent's other Tilde tools
      and its bundled tools (``bundled``, from ``define_tools``, audited and published as
      ``with_tilde_tools`` does), added to the agent's own for this run (Agno's per-run tools,
      as AG-UI frontend tools use them).
    - Steering: a tool hook on those tools appends input steered while the agent works to the
      tool result the model reads next. Agno has no per-step message hook, so steering waits for
      a Tilde tool call; an agent-level ``tool_hooks`` replaces this hook.
    - ``run_id``: registered so a stop or suspension cancels the run (``agno.run.cancel``).
    - Dynamic instructions, description or system message stamp this invocation's model calls.
    - Registry skills: through ``TildeSkills`` when the agent uses it, else as ``list_skills``
      and ``read_skill`` tools with their summary in the system message.
    """
    tools = await with_tilde_tools(
        ctx,
        bundled.tools if bundled else [],
        options=bundled.options if bundled else None,
    )
    if isinstance(agent.skills, TildeSkills):
        directory = LocalSkills(await ctx.skills.directory(), validate=False)
        _registry.set({skill.name: skill for skill in await asyncio.to_thread(directory.load)})
    else:
        summary = await ctx.skills.summary()
        if summary:
            skill_tools = convert_to_agno_tools(ctx.skills.tools())
            # Agno adds a function's instructions to the system message for the run.
            skill_tools[0].instructions = summary  # list_skills
            skill_tools[0].add_instructions = True
            tools += skill_tools
    steering = _steering(ctx)
    for tool in tools:
        tool.tool_hooks = [steering]

    fields = (agent.instructions, agent.description, agent.system_message)
    name = agent.name or (_variable(agent) if any(callable(f) for f in fields) else None)
    for prompt in agent_prompts(agent, name, "", lambda _: None) if name else []:
        if prompt.format == PROMPT_FORMAT_DYNAMIC:
            ctx.activate_prompt(prompt.name, prompt.hash)

    run_id = str(uuid.uuid4())
    ctx.cancellation.on_abort(lambda: cancel_run(run_id))
    run_context = RunContext(run_id=run_id, session_id=ctx.thread_id, client_tools=tools)
    return {"run_id": run_id, "session_id": ctx.thread_id, "run_context": run_context}


def _steering(ctx: AgentContext) -> Callable[..., Awaitable[Any]]:
    async def steer(function_call: Callable[..., Awaitable[Any]], arguments: dict[str, Any]) -> Any:
        result = await function_call(**arguments)
        inputs = ctx.take_inputs()
        if not inputs:
            return result
        text = result if isinstance(result, str) else json.dumps(result, default=str)
        steered = "\n".join(f"- {steered.text}" for steered in inputs)
        return f"{text}\n\nNew messages arrived while you were working:\n{steered}"

    return steer


def _variable(agent: Agent | Team) -> str | None:
    # Discovery names an unnamed agent after its module-level variable; find the same one.
    for module in list(sys.modules.values()):
        for key, value in list(getattr(module, "__dict__", {}).items()):
            if value is agent and not key.startswith("_"):
                return key
    return None
