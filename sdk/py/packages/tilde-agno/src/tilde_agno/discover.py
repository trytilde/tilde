"""``tilde deploy`` discovery for Agno (entry point ``tilde.discover``).

A module-level ``Agent`` or ``Team`` registers, named after ``name`` (else its variable):
``<name>/instructions`` (``/<n>`` from 1 for a list), ``<name>/description`` and
``<name>/system_message``. Text is plain, or braces when it holds a ``{var}`` Agno resolves
from session state and dependencies (``resolve_in_context``, on by default); a function is
dynamic with its source as the template, stamped at runtime by ``tilde_agno``. Folders of
``LocalSkills`` loaders in ``skills`` ship with the deployment. Functions and toolkits in
``tools`` are declared as ``with_tilde_tools`` publishes them, as is a ``define_tools`` value of
Agno tools, or of plain functions when the project imports Agno. Nothing is run.
"""

from __future__ import annotations

import inspect
import re
import sys
from collections.abc import Callable, Mapping
from pathlib import Path
from typing import Any

from agno.agent import Agent
from agno.models.message import Message
from agno.skills import LocalSkills, Skills
from agno.team import Team
from agno.tools.function import Function
from agno.tools.toolkit import Toolkit

from tilde import BundledOptions, BundledTools
from tilde.discovery import (
    PROMPT_FORMAT_BRACES,
    PROMPT_FORMAT_DYNAMIC,
    PROMPT_FORMAT_PLAIN,
    Discovered,
    DiscoveryContext,
    declared_prompt,
    declared_tool,
)
from tilde.management.v1.deployments_pb2 import DeclaredPrompt, DeclaredTool
from tilde.prompts import valid_prompt_name
from tilde.skills import SkillDefinition, read_skill, read_skills
from tilde_agno.tools import describe_tool, functions_of, processed

_VARIABLE = re.compile(r"\{[A-Za-z_][A-Za-z0-9_]*\}")


def discover(value: Any, context: DiscoveryContext) -> Discovered | None:
    if isinstance(value, BundledTools):
        if not value.tools or not all(_readable(tool) for tool in value.tools):
            return None
        # Plain functions alone could be another framework's.
        ours = {"agno", "tilde_agno"} & context.imported or any(
            not inspect.isroutine(tool) for tool in value.tools
        )
        if not ours:
            return None
        return Discovered(tools=_tools(value.tools, value.options, f"{context.origin()}.tools"))
    if not isinstance(value, Agent | Team):
        return None
    file = Path(sys.modules[context.module].__file__ or "")
    origin = f"{context.relative(file)}#{context.name}"
    # `tools` may also be a factory called per run; dict tool definitions are skipped too.
    tools = list(value.tools) if isinstance(value.tools, list | tuple) else []
    return Discovered(
        prompts=agent_prompts(value, value.name or context.name, origin, context.warn),
        skills=_skills(value.skills, f"{origin}.skills", context),
        tools=_tools([tool for tool in tools if _readable(tool)], None, f"{origin}.tools"),
    )


def _readable(tool: Any) -> bool:
    return isinstance(tool, Function | Toolkit) or inspect.isroutine(tool)


def _tools(
    tools: list[Any], options: Mapping[str, BundledOptions] | None, origin: str
) -> list[DeclaredTool]:
    declared = []
    for function in functions_of(tools):
        spec = describe_tool(processed(function), options)
        declared.append(declared_tool(spec, f"{origin}.{spec.name}"))
    return declared


def agent_prompts(
    agent: Agent | Team, name: str, origin: str, warn: Callable[[str], None]
) -> list[DeclaredPrompt]:
    """The agent's prompts; ``tilde_agno`` calls this again at runtime for the stamps."""
    if not valid_prompt_name(name):
        warn(f"Agno agent {name!r} cannot name a prompt; set name= ({origin})")
        return []
    # (prompt name suffix, attribute for the origin, value)
    fields: list[tuple[str, str, Any]] = []
    if isinstance(agent.instructions, list) and len(agent.instructions) > 1:
        for index, text in enumerate(agent.instructions):
            fields.append((f"instructions/{index + 1}", f"instructions[{index}]", text))
    elif isinstance(agent.instructions, list):
        fields.append(("instructions", "instructions", next(iter(agent.instructions), None)))
    else:
        fields.append(("instructions", "instructions", agent.instructions))
    fields.append(("description", "description", agent.description))
    system = agent.system_message
    system = system.content if isinstance(system, Message) else system
    fields.append(("system_message", "system_message", system))
    prompts = []
    for field, attribute, value in fields:
        where = f"{origin}.{attribute}"
        if isinstance(value, str) and value:
            braces = agent.resolve_in_context and _VARIABLE.search(value)
            format = PROMPT_FORMAT_BRACES if braces else PROMPT_FORMAT_PLAIN
            prompts.append(declared_prompt(f"{name}/{field}", value, format, where))
        elif callable(value):
            try:
                source = inspect.getsource(value)
            except (OSError, TypeError):
                warn(f"{where}: the source of {name}/{field} could not be read; not registered")
                continue
            prompts.append(declared_prompt(f"{name}/{field}", source, PROMPT_FORMAT_DYNAMIC, where))
        elif value is not None and value != "":
            warn(f"{where}: unsupported {type(value).__name__}; not registered")
    return prompts


def _skills(skills: Skills | None, origin: str, context: DiscoveryContext) -> list[SkillDefinition]:
    found: list[SkillDefinition] = []
    for loader in skills.loaders if skills is not None else []:
        if not isinstance(loader, LocalSkills):
            context.warn(f"{origin}: {type(loader).__name__} skills are not shipped")
            continue
        # LocalSkills takes a skill folder or a directory of them, resolved at construction.
        path = loader.path
        found.extend([read_skill(path)] if (path / "SKILL.md").is_file() else read_skills(path))
    return found
