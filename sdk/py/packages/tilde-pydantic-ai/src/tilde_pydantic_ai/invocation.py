"""The per-invocation pieces of a Pydantic AI run, as ``agent.run`` keyword arguments."""

from __future__ import annotations

import dataclasses
from dataclasses import dataclass, field
from typing import Any

from pydantic_ai import CancellationToken, RunContext
from pydantic_ai.capabilities import AbstractCapability
from pydantic_ai.messages import (
    ModelMessage,
    ModelMessagesTypeAdapter,
    ModelRequest,
    ToolReturnPart,
)
from pydantic_ai.models import ModelRequestContext
from pydantic_ai.toolsets import AbstractToolset, CombinedToolset, FunctionToolset

from tilde import AgentContext, BundledTools, Resumed
from tilde.discovery import PROMPT_FORMAT_DYNAMIC
from tilde_pydantic_ai.discover import agent_prompts
from tilde_pydantic_ai.tools import convert_to_pydantic_ai_tools, with_tilde_tools


@dataclass
class TildeInvocation(AbstractCapability[Any]):
    """Per-run capability: Tilde tools, the skills summary, steering and dynamic prompt stamps.

    Steering: after every graph node, newly steered input is enqueued (``RunContext.enqueue``)
    and Pydantic AI delivers it as a user message with the next model request, or redirects a
    run that would otherwise end into one more request.

    Suspension: before each model request, a Tilde tool call still running after about a minute
    (it answered pending) or Tilde's deadline saves the message history
    (``ModelMessagesTypeAdapter``) and ends the invocation. The run's next invocation replaces
    the request's history with it, each pending tool return replaced by the call's outcome
    (Pydantic AI keeps the hook's messages as the run's history).
    """

    context: AgentContext
    toolset: AbstractToolset[Any]
    skills: str | None = None
    restored: bool = False
    latest: list[ModelMessage] = field(default_factory=list)

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

    async def before_model_request(
        self, ctx: RunContext[Any], request_context: ModelRequestContext
    ) -> ModelRequestContext:
        messages = request_context.messages
        if self.context.resumed is not None and not self.restored:
            self.restored = True
            messages = _restore(self.context.resumed)
            request_context = dataclasses.replace(request_context, messages=messages)
        self.latest = messages
        if self.context.should_suspend:
            await self.context.suspend(_json(messages))
        return request_context

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
    ctx.enable_suspension(lambda: _json(capability.latest))
    return {"capabilities": [capability], "cancellation_token": token}


def _json(messages: list[ModelMessage]) -> Any:
    return ModelMessagesTypeAdapter.dump_python(messages, mode="json")


def _restore(resumed: Resumed) -> list[ModelMessage]:
    """The saved history, each pending tool return replaced by the call's outcome."""
    messages = ModelMessagesTypeAdapter.validate_python(resumed.state)
    for message in messages:
        if not isinstance(message, ModelRequest):
            continue
        for index, part in enumerate(message.parts):
            outcome = (
                resumed.outcomes.get(part.tool_call_id)
                if isinstance(part, ToolReturnPart)
                else None
            )
            if outcome is not None:
                content = outcome.output if outcome.error is None else {"error": outcome.error}
                message.parts[index] = dataclasses.replace(part, content=content)
    return messages
