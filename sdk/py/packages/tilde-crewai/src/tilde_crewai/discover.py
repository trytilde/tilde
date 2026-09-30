"""``tilde deploy`` discovery for CrewAI (entry point ``tilde.discover``).

``@CrewBase`` classes keep their YAML config paths relative to the class file
(``base_directory`` / ``original_agents_config_path``). The YAML is read raw, before CrewAI
interpolates inputs: each agent ``role``/``goal``/``backstory`` becomes the prompt
``agents/<key>/<field>`` and each task ``description``/``expected_output`` becomes
``tasks/<key>/<field>``, in braces format when it holds a ``{variable}``. Skill search paths
listed under an agent's ``skills:`` resolve from the working directory, as CrewAI resolves them.
Nothing is instantiated, so tools given to agents in ``@agent`` methods are not seen; a
``define_tools`` value of CrewAI tools, and the tools of a module-level agent, are declared as
``with_tilde_tools`` publishes them.
"""

from __future__ import annotations

import re
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import yaml
from crewai.agents.agent_builder.base_agent import BaseAgent
from crewai.tools import BaseTool

from tilde import BundledOptions, BundledTools, SkillDefinition, SkillFile
from tilde.discovery import (
    PROMPT_FORMAT_BRACES,
    PROMPT_FORMAT_PLAIN,
    Discovered,
    DiscoveryContext,
    declared_prompt,
    declared_tool,
)
from tilde.management.v1.deployments_pb2 import DeclaredTool
from tilde.prompts import valid_prompt_name
from tilde.skills import read_skills
from tilde_crewai.tools import describe_tool

# CrewAI's own interpolation pattern (crewai.utilities.string_utils).
_VARIABLE = re.compile(r"\{([A-Za-z_][A-Za-z0-9_\-]*)}")
_FIELDS = {"agents": ("role", "goal", "backstory"), "tasks": ("description", "expected_output")}


def discover(value: Any, context: DiscoveryContext) -> Discovered | None:
    if isinstance(value, BundledTools):
        if not value.tools or not all(isinstance(tool, BaseTool) for tool in value.tools):
            return None
        return Discovered(tools=_tools(value.tools, value.options, f"{context.origin()}.tools"))
    if isinstance(value, BaseAgent):
        context.warn(
            f"CrewAI agent {context.module}.{context.name} is built in code; declare it in a "
            "@CrewBase agents.yaml to register its prompts"
        )
        tools = [tool for tool in value.tools or [] if isinstance(tool, BaseTool)]
        return Discovered(tools=_tools(tools, None, f"{context.origin()}.tools"))
    cls = value if isinstance(value, type) else type(value)
    if not getattr(cls, "is_crew_class", False) or not hasattr(cls, "base_directory"):
        return None
    found = Discovered()
    for kind, attribute in (
        ("agents", "original_agents_config_path"),
        ("tasks", "original_tasks_config_path"),
    ):
        path = getattr(cls, attribute, None)
        explicit = getattr(cls, f"{kind}_config", None) is not None
        if not isinstance(path, str):
            if explicit:
                context.warn(f"{cls.__name__}.{kind}_config is not a YAML path; not registered")
            continue
        file = Path(cls.base_directory) / path
        if not file.is_file():
            if explicit:
                context.warn(f"{cls.__name__}: {kind} config {file} does not exist")
            continue
        config = yaml.safe_load(file.read_text(encoding="utf-8")) or {}
        if not isinstance(config, dict):
            context.warn(f"{context.relative(file)} is not a mapping of {kind}")
            continue
        _read(kind, config, context.relative(file), found, context)
    return found


def _tools(
    tools: list[BaseTool], options: Mapping[str, BundledOptions] | None, origin: str
) -> list[DeclaredTool]:
    specs = [describe_tool(tool, options) for tool in tools]
    return [declared_tool(spec, f"{origin}.{spec.name}") for spec in specs]


def _read(
    kind: str, config: dict[str, Any], origin: str, found: Discovered, context: DiscoveryContext
) -> None:
    for key, entry in config.items():
        if not isinstance(entry, dict):
            continue
        for field in _FIELDS[kind]:
            text = entry.get(field)
            if not isinstance(text, str) or not text:
                continue
            name = f"{kind}/{key}/{field}"
            if not valid_prompt_name(name):
                context.warn(f"{origin}#{key}: {key!r} cannot name a prompt")
                break
            format = PROMPT_FORMAT_BRACES if _VARIABLE.search(text) else PROMPT_FORMAT_PLAIN
            found.prompts.append(declared_prompt(name, text, format, f"{origin}#{key}.{field}"))
        if kind == "agents":
            for skill in entry.get("skills") or []:
                found.skills.extend(_skills(skill, f"{origin}#{key}.skills", context))


def _skills(skill: Any, origin: str, context: DiscoveryContext) -> list[SkillDefinition]:
    if not isinstance(skill, str):
        context.warn(f"{origin}: only skill paths and inline SKILL.md are registered")
        return []
    if skill.startswith("@"):
        context.warn(f"{origin}: registry skill {skill} is not shipped with the deployment")
        return []
    if skill.lstrip().startswith("---"):
        name = re.search(r"^name:\s*['\"]?([^'\"\n]+)", skill, re.MULTILINE)
        if name is None:
            context.warn(f"{origin}: inline SKILL.md has no name")
            return []
        return [SkillDefinition(name[1].strip(), [SkillFile("SKILL.md", skill.encode())])]
    return read_skills(context.root / skill)
