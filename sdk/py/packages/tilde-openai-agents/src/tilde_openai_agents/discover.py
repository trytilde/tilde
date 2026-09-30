"""``tilde deploy`` discovery for the OpenAI Agents SDK (entry point ``tilde.discover``).

A module-level ``Agent`` contributes itself and every agent reachable through its handoffs and
agent tools (``Agent.as_tool``), each once:

- ``<name>/instructions``: a string is plain text; a function is dynamic with its source.
- ``<name>/handoff_description``: plain text.
- A hosted ``prompt`` (``{"id", "version"}``) lives in the OpenAI dashboard and is not versioned
  by Tilde; it is reported as a warning.
- Skill folders the SDK loads from disk: local ``ShellTool`` environment skills and a
  ``SandboxAgent``'s ``Skills(from_=LocalDir(...))`` / ``lazy_from=LocalDirLazySkillSource``.
  Relative paths resolve from the working directory, as the SDK resolves them.
- ``FunctionTool``s in ``tools`` (agent tools included), declared as ``with_tilde_tools``
  publishes them, as is a ``define_tools`` value of ``FunctionTool``s. Hosted and shell tools
  are not.

Handoff targets and agent-tool agents are only reachable through private fields
(``Handoff._agent_ref``, ``FunctionTool._agent_instance``, openai-agents 0.22); an unreadable
one is reported as a warning.
"""

from __future__ import annotations

import inspect
import re
import sys
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from agents import Agent, FunctionTool, Handoff, ShellTool
from agents.sandbox import SandboxAgent
from agents.sandbox.capabilities import LocalDirLazySkillSource, Skills
from agents.sandbox.entries import LocalDir

from tilde import BundledTools
from tilde.discovery import (
    PROMPT_FORMAT_DYNAMIC,
    PROMPT_FORMAT_PLAIN,
    Discovered,
    DiscoveryContext,
    declared_prompt,
    declared_tool,
)
from tilde.prompts import canonical_json, prompt_hash
from tilde.skills import read_skill, read_skills
from tilde_openai_agents.tools import describe_tool

_UNSAFE = re.compile(r"[^A-Za-z0-9._-]")


def prompt_name(agent: Agent[Any], field: str) -> str:
    """Prompt names allow ``[A-Za-z0-9._/-]``; agent names are free text."""
    return f"{_UNSAFE.sub('-', agent.name)[:96] or 'agent'}/{field}"


def reachable_agents(root: Agent[Any], warn: Any = None) -> Iterator[tuple[str, Agent[Any]]]:
    """``root`` and every agent reachable through handoffs and agent tools, each once, with
    the attribute path from ``root`` (``handoffs.billing``, ``tools.lookup``)."""
    seen: set[int] = set()
    pending: list[tuple[str, Agent[Any]]] = [("", root)]
    while pending:
        path, agent = pending.pop(0)
        if id(agent) in seen:
            continue
        seen.add(id(agent))
        yield path, agent
        prefix = f"{path}." if path else ""
        for item in agent.handoffs:
            target = item
            if isinstance(item, Handoff):
                ref = getattr(item, "_agent_ref", None)
                target = ref() if callable(ref) else None
            if isinstance(target, Agent):
                pending.append((f"{prefix}handoffs.{target.name}", target))
            elif warn is not None:
                warn(f"handoff {getattr(item, 'agent_name', '?')} of agent {agent.name}")
        for tool in agent.tools:
            if not getattr(tool, "_is_agent_tool", False):
                continue
            target = getattr(tool, "_agent_instance", None)
            if isinstance(target, Agent):
                pending.append((f"{prefix}tools.{tool.name}", target))
            elif warn is not None:
                warn(f"agent tool {tool.name} of agent {agent.name}")


def dynamic_source(instructions: Any) -> str | None:
    """The source of an instructions function (a callable object's class), or None when Python
    cannot read it."""
    for candidate in (instructions, type(instructions)):
        try:
            return inspect.getsource(candidate)
        except (OSError, TypeError):
            continue
    return None


def instructions_stamp(agent: Agent[Any]) -> tuple[str, str] | None:
    """``(name, hash)`` of an agent's dynamic instructions, as ``tilde deploy`` registers them."""
    if not callable(agent.instructions):
        return None
    source = dynamic_source(agent.instructions)
    if source is None:
        return None
    return prompt_name(agent, "instructions"), prompt_hash(source, {}, canonical_json({}))


def discover(value: Any, context: DiscoveryContext) -> Discovered | None:
    if isinstance(value, BundledTools):
        if not value.tools or not all(isinstance(tool, FunctionTool) for tool in value.tools):
            return None
        origin = f"{context.origin()}.tools"
        return Discovered(
            tools=[
                declared_tool(describe_tool(tool, value.options), f"{origin}.{tool.name}")
                for tool in value.tools
            ]
        )
    if not isinstance(value, Agent):
        return None
    module = sys.modules.get(context.module)
    file = getattr(module, "__file__", None)
    origin = f"{context.relative(Path(file)) if file else context.module}#{context.name}"
    found = Discovered()

    def unreadable(what: str) -> None:
        context.warn(f"{origin}: {what} could not be read; its prompts are not registered")

    for path, agent in reachable_agents(value, unreadable):
        at = f"{origin}.{path}" if path else origin
        _prompts(agent, at, found, context)
        _skills(agent, at, found, context)
        found.tools += [
            declared_tool(describe_tool(tool), f"{at}.tools.{tool.name}")
            for tool in agent.tools
            if isinstance(tool, FunctionTool)
        ]
    return found


def _prompts(agent: Agent[Any], at: str, found: Discovered, context: DiscoveryContext) -> None:
    instructions = agent.instructions
    if isinstance(instructions, str) and instructions:
        found.prompts.append(
            declared_prompt(
                prompt_name(agent, "instructions"),
                instructions,
                PROMPT_FORMAT_PLAIN,
                f"{at}.instructions",
            )
        )
    elif callable(instructions):
        source = dynamic_source(instructions)
        if source is None:
            context.warn(f"{at}.instructions: the function's source cannot be read")
        else:
            found.prompts.append(
                declared_prompt(
                    prompt_name(agent, "instructions"),
                    source,
                    PROMPT_FORMAT_DYNAMIC,
                    f"{at}.instructions",
                )
            )
    if agent.handoff_description:
        found.prompts.append(
            declared_prompt(
                prompt_name(agent, "handoff_description"),
                agent.handoff_description,
                PROMPT_FORMAT_PLAIN,
                f"{at}.handoff_description",
            )
        )
    if agent.prompt is not None:
        hosted = agent.prompt.get("id") if isinstance(agent.prompt, dict) else "(dynamic)"
        context.warn(
            f"{at}.prompt: OpenAI-hosted prompt {hosted} is versioned by OpenAI, not by Tilde"
        )


def _skills(agent: Agent[Any], at: str, found: Discovered, context: DiscoveryContext) -> None:
    for tool in agent.tools:
        if not isinstance(tool, ShellTool) or not isinstance(tool.environment, dict):
            continue
        if tool.environment.get("type") != "local":
            continue
        for skill in tool.environment.get("skills") or []:
            directory = context.root / skill["path"]
            if (directory / "SKILL.md").is_file():
                found.skills.append(read_skill(directory))
            else:
                context.warn(f"{at}.tools.{tool.name}: skill {skill['name']} has no SKILL.md")
    if not isinstance(agent, SandboxAgent):
        return
    for capability in agent.capabilities:
        if not isinstance(capability, Skills):
            continue
        source = capability.from_
        if isinstance(capability.lazy_from, LocalDirLazySkillSource):
            source = capability.lazy_from.source
        if isinstance(source, LocalDir) and source.src is not None:
            found.skills.extend(read_skills(context.root / source.src))
        else:
            context.warn(f"{at}.capabilities: only LocalDir skills are registered")
