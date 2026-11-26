"""The per-invocation pieces of an OpenAI Agents SDK run.

``Runner.run`` takes the agent positionally and everything else as options, so the helper
returns both: a clone of the module-level agent carrying this invocation's tools and skills,
and a ``RunConfig`` whose ``call_model_input_filter`` runs before every model call.
"""

from __future__ import annotations

import dataclasses
import inspect
import json
from pathlib import Path
from typing import Any

from agents import Agent, RunConfig, RunContextWrapper, ShellTool, TResponseInputItem
from agents.run_config import CallModelData, CallModelInputFilter, ModelInputData

from tilde import AgentContext, BundledTools, Resumed
from tilde_openai_agents.discover import instructions_stamp, reachable_agents
from tilde_openai_agents.tools import convert_to_openai_agents_tools, with_tilde_tools


async def tilde_openai_agents(
    ctx: AgentContext,
    agent: Agent[Any],
    *,
    bundled: BundledTools | None = None,
    run_config: RunConfig | None = None,
) -> tuple[Agent[Any], RunConfig]:
    """``agent, config = await tilde_openai_agents(ctx, agent, bundled=TOOLS)`` then
    ``await Runner.run(agent, input, run_config=config, max_turns=8)``.

    - Tools: the current channel's tools, the agent's other Tilde tools and its bundled tools
      (``bundled``, from ``define_tools``, audited and published as ``with_tilde_tools`` does)
      are added to the returned clone of ``agent``.
    - Skills assigned in the registry: with a local ``ShellTool`` they are added to its
      environment ``skills`` (written to disk by ``ctx.skills.directory()``); otherwise the
      ``list_skills``/``read_skill`` tools are added and ``ctx.skills.summary()`` is appended
      to the instructions.
    - Before every model call: cancellation is checked, the calling agent's dynamic
      instructions stamp this invocation's inference calls, and steering input sent while the
      agent works becomes user messages. ``run_config``'s own filter, if any, runs first.
    - Suspension: a Tilde tool call still running after about a minute (it answered pending) or
      Tilde's deadline saves the items before the next model call and ends the invocation; the
      run's next invocation continues from them, each pending output replaced by the call's
      outcome.

    Handoff targets and agent tools run as defined; only ``agent`` gets Tilde's tools.
    """
    stamps = {
        candidate.name: stamp
        for _, candidate in reachable_agents(agent)
        if (stamp := instructions_stamp(candidate)) is not None
    }
    own = await with_tilde_tools(
        ctx,
        bundled.tools if bundled else [],
        options=bundled.options if bundled else None,
    )
    tools = [*agent.tools, *own]
    instructions = agent.instructions
    shell = next(
        (
            tool
            for tool in tools
            if isinstance(tool, ShellTool) and (tool.environment or {}).get("type") == "local"
        ),
        None,
    )
    if shell is not None:
        environment: dict[str, Any] = dict(shell.environment or {})
        environment["skills"] = [*environment.get("skills", []), *await _local_skills(ctx)]
        tools[tools.index(shell)] = dataclasses.replace(shell, environment=environment)
    else:
        summary = await ctx.skills.summary()
        if summary:
            tools.extend(convert_to_openai_agents_tools(ctx.skills.tools()))
            instructions = _with_summary(instructions, summary)
    config = dataclasses.replace(run_config) if run_config is not None else RunConfig()
    config.call_model_input_filter = _input_filter(ctx, stamps, config.call_model_input_filter)
    return agent.clone(tools=tools, instructions=instructions), config


async def _local_skills(ctx: AgentContext) -> list[dict[str, str]]:
    registry = [skill for skill in await ctx.skills.list() if not skill.deployed]
    if not registry:
        return []
    root = Path(await ctx.skills.directory())
    names = [skill.name for skill in registry]
    skills = []
    for skill in registry:
        # `directory()` names a folder `<source>-<name>` only when two sources share a name.
        folder = f"{skill.source}-{skill.name}" if names.count(skill.name) > 1 else skill.name
        skills.append(
            {"name": skill.name, "description": skill.description, "path": str(root / folder)}
        )
    return skills


def _with_summary(instructions: Any, summary: str) -> Any:
    if instructions is None:
        return summary
    if isinstance(instructions, str):
        return f"{instructions}\n\n{summary}"

    # The SDK requires exactly (context, agent).
    async def with_summary(context: RunContextWrapper[Any], agent: Agent[Any]) -> str:
        text = instructions(context, agent)
        if inspect.isawaitable(text):
            text = await text
        return f"{text}\n\n{summary}"

    return with_summary


def _input_filter(
    ctx: AgentContext,
    stamps: dict[str, tuple[str, str]],
    previous: CallModelInputFilter | None,
) -> CallModelInputFilter:
    # The SDK sends the filtered input for one call only and keeps its own history, so steered
    # messages are remembered with their position and re-inserted on every later call. That
    # assumes the SDK resends the history; with server-managed conversations
    # (`previous_response_id`, `conversation_id`) it sends only new items.
    steered: list[tuple[int, list[TResponseInputItem]]] = []
    # Suspension: the items at the last model call, and a resumed run's saved items, which
    # replace the run's own input (the runner keeps its input as the prefix of every later call).
    initial: int | None = None
    restored: list[TResponseInputItem] | None = None
    latest: list[TResponseInputItem] = []
    ctx.enable_suspension(lambda: _json(latest))

    async def filter(data: CallModelData[Any]) -> ModelInputData:
        nonlocal initial, restored, latest
        if previous is not None:
            updated = previous(data)
            data.model_data = await updated if inspect.isawaitable(updated) else updated
        ctx.check()
        stamp = stamps.get(data.agent.name)
        if stamp is not None:
            ctx.activate_prompt(*stamp)
        items = list(data.model_data.input)
        if initial is None:
            initial = len(items)
            resumed = ctx.take_resumed()
            if resumed is not None:
                restored = _restore(resumed)
        if restored is not None:
            items = [*restored, *items[initial:]]
        inputs = ctx.take_inputs()
        if inputs:
            steered.append(
                (len(items), [{"role": "user", "content": input.text} for input in inputs])
            )
        offset = 0
        for position, added in steered:
            at = min(position + offset, len(items))
            items[at:at] = added
            offset += len(added)
        latest = items
        if ctx.should_suspend:
            await ctx.suspend(_json(items))
        return ModelInputData(input=items, instructions=data.model_data.instructions)

    return filter


def _json(items: list[TResponseInputItem]) -> Any:
    # Items the runner generated are plain dicts already; pydantic ones dump themselves.
    return json.loads(json.dumps(items, default=lambda item: item.model_dump(exclude_unset=True)))


def _restore(resumed: Resumed) -> list[TResponseInputItem]:
    """The saved items, each pending function call output replaced by the call's outcome."""
    items: list[TResponseInputItem] = []
    for item in resumed.state:
        outcome = (
            resumed.outcomes.get(item.get("call_id", ""))
            if item.get("type") == "function_call_output"
            else None
        )
        if outcome is not None:
            failed = outcome.error is not None
            output = f"Error: {outcome.error}" if failed else json.dumps(outcome.output)
            item = {**item, "output": output}
        items.append(item)
    return items
