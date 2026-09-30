"""The per-invocation pieces of a Pydantic AI run, as ``agent.run`` keyword arguments."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from pydantic_ai import CancellationToken, RunContext
from pydantic_ai.capabilities import AbstractCapability
from pydantic_ai.toolsets import AbstractToolset, CombinedToolset, FunctionToolset

from tilde import AgentContext, BundledTools
from tilde.discovery import PROMPT_FORMAT_DYNAMIC
from tilde_pydantic_ai.discover import agent_prompts
from tilde_pydantic_ai.tools import convert_to_pydantic_ai_tools, with_tilde_tools


@dataclass
class TildeInvocation(AbstractCapability[Any]):
    """Per-run capability: Tilde tools, the skills summary, steering and dynamic prompt stamps.

    Steering: after every graph node, newly steered input is enqueued (``RunContext.enqueue``)
    and Pydantic AI delivers it as a user message with the next model request, or redirects a
    run that would otherwise end into one more request.
    """

    context: AgentContext
    toolset: AbstractToolset[Any]
    skills: str | None = None

    @classmethod
    def get_serialization_name(cls) -> str | None:
        return None  # holds the live invocation

    def get_toolset(self) -> AbstractToolset[Any]:
        return self.toolset

    def get_instructions(self) -> str | None:
        return self.skills

    async def before_run(self, ctx: RunContext[Any]) -> None:
        # `agent.run` has named an unnamed agent after its variable by now, as discovery did.
        agent = ctx.agent
        if agent is None or agent.name is None:
            return
        for prompt in agent_prompts(agent, agent.name, "", lambda _: None):
            if prompt.format == PROMPT_FORMAT_DYNAMIC:
                self.context.activate_prompt(prompt.name, prompt.hash)

    async def after_node_run(self, ctx: RunContext[Any], *, node: Any, result: Any) -> Any:
        for steered in self.context.take_inputs():
            ctx.enqueue(steered.text)
        return result


async def tilde_pydantic_ai(
    ctx: AgentContext, *, bundled: BundledTools | None = None, skills: bool = True
) -> dict[str, Any]:
    """``agent.run(..., **await tilde_pydantic_ai(ctx, bundled=TOOLS))``: the current channel's
    tools, the agent's other Tilde tools, its bundled tools (``bundled``, from ``define_tools``,
    audited and published as ``with_tilde_tools`` does), the invocation's cancellation,
    steering, dynamic prompt stamps and, with ``skills``, the agent's skills (deployed and
    registry-assigned) as ``list_skills``/``read_skill`` tools with a summary in the
    instructions, since Pydantic AI has no native skills.
    """
    toolset = await with_tilde_tools(
        ctx,
        bundled.tools if bundled else [],
        options=bundled.options if bundled else None,
    )
    summary = await ctx.skills.summary() if skills else ""
    if summary:
        toolset = CombinedToolset(
            [toolset, FunctionToolset(convert_to_pydantic_ai_tools(ctx.skills.tools()))]
        )
    token = CancellationToken()
    ctx.cancellation.on_abort(token.cancel)
    capability = TildeInvocation(ctx, toolset, summary or None)
    return {"capabilities": [capability], "cancellation_token": token}
