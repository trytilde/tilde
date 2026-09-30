"""``tilde deploy`` discovery for Pydantic AI (entry point ``tilde.discover``).

A module-level ``Agent`` registers its instructions and system prompts, named
``<agent name or variable>/instructions`` and ``<agent>/system_prompt`` (``/<n>`` from 1 when
there are several): literal text is plain, ``TemplateStr`` (Handlebars) is mustache and a
function is dynamic with its source as the template; ``tilde_pydantic_ai`` stamps the dynamic ones
on the run's model calls. Pydantic AI keeps them in private fields (``_instructions``,
``_system_prompts``, ``_system_prompt_functions``, as of 2.43); an agent whose fields cannot be
read is reported in the inventory.

Tools are declared as ``with_tilde_tools`` publishes them: the agent's function tools
(``tools=``, ``@agent.tool``; the private ``_function_toolset``) and a ``define_tools`` value of
Pydantic AI ``Tool``s, or of plain functions when the project imports Pydantic AI.
"""

from __future__ import annotations

import inspect
import sys
from collections.abc import Callable, Mapping
from pathlib import Path
from typing import Any

from pydantic_ai import Agent, Tool
from pydantic_ai.messages import InstructionPart
from pydantic_ai.template import TemplateStr

from tilde import BundledOptions, BundledTools
from tilde.discovery import (
    PROMPT_FORMAT_DYNAMIC,
    PROMPT_FORMAT_MUSTACHE,
    PROMPT_FORMAT_PLAIN,
    Discovered,
    DiscoveryContext,
    declared_prompt,
    declared_tool,
)
from tilde.management.v1.deployments_pb2 import DeclaredPrompt, DeclaredTool
from tilde.prompts import valid_prompt_name
from tilde_pydantic_ai.tools import describe_tool


def discover(value: Any, context: DiscoveryContext) -> Discovered | None:
    if isinstance(value, BundledTools):
        return _bundled(value, context)
    if not isinstance(value, Agent):
        return None
    file = Path(sys.modules[context.module].__file__ or "")
    origin = f"{context.relative(file)}#{context.name}"
    name = value.name or context.name
    try:
        tools = list(value._function_toolset.tools.values())
    except AttributeError:
        context.warn(f"Pydantic AI agent {name}: tools could not be read ({origin})")
        tools = []
    return Discovered(
        prompts=agent_prompts(value, name, origin, context.warn),
        tools=_declared(tools, None, f"{origin}.tools"),
    )


def _bundled(value: BundledTools, context: DiscoveryContext) -> Discovered | None:
    if not all(isinstance(tool, Tool) or inspect.isroutine(tool) for tool in value.tools):
        return None
    # Plain functions alone could be another framework's.
    ours = {"pydantic_ai", "tilde_pydantic_ai"} & context.imported or any(
        isinstance(tool, Tool) for tool in value.tools
    )
    if not value.tools or not ours:
        return None
    tools = [tool if isinstance(tool, Tool) else Tool(tool) for tool in value.tools]
    return Discovered(tools=_declared(tools, value.options, f"{context.origin()}.tools"))


def _declared(
    tools: list[Tool[Any]], options: Mapping[str, BundledOptions] | None, origin: str
) -> list[DeclaredTool]:
    return [declared_tool(describe_tool(tool, options), f"{origin}.{tool.name}") for tool in tools]


def agent_prompts(
    agent: Agent[Any, Any], name: str, origin: str, warn: Callable[[str], None]
) -> list[DeclaredPrompt]:
    """The agent's prompts; ``tilde_pydantic_ai`` calls this again at runtime for the stamps."""
    try:
        instructions = [sourced.instruction for sourced in agent._instructions]
        system = [*agent._system_prompts]
        system += [runner.function for runner in agent._system_prompt_functions]
    except AttributeError:
        warn(f"Pydantic AI agent {name}: instructions could not be read ({origin})")
        return []
    if not valid_prompt_name(name):
        warn(f"Pydantic AI agent {name!r} cannot name a prompt; set Agent(name=...) ({origin})")
        return []
    prompts = []
    for kind, values in (("instructions", instructions), ("system_prompt", system)):
        for index, value in enumerate(values, 1):
            key = f"{name}/{kind}" if len(values) == 1 else f"{name}/{kind}/{index}"
            where = f"{origin}.{kind}" if len(values) == 1 else f"{origin}.{kind}[{index - 1}]"
            prompt = _prompt(key, value, where, warn)
            if prompt is not None:
                prompts.append(prompt)
    return prompts


def _prompt(
    name: str, value: Any, origin: str, warn: Callable[[str], None]
) -> DeclaredPrompt | None:
    if isinstance(value, str):
        return declared_prompt(name, value, PROMPT_FORMAT_PLAIN, origin) if value else None
    if isinstance(value, InstructionPart):
        return declared_prompt(name, value.content, PROMPT_FORMAT_PLAIN, origin)
    # TemplateStr is callable too; str() is its Handlebars source.
    if isinstance(value, TemplateStr):
        return declared_prompt(name, str(value), PROMPT_FORMAT_MUSTACHE, origin)
    if callable(value):
        try:
            source = inspect.getsource(value)
        except (OSError, TypeError):
            warn(f"{origin}: the source of {name} could not be read; not registered")
            return None
        return declared_prompt(name, source, PROMPT_FORMAT_DYNAMIC, origin)
    warn(f"{origin}: unsupported instruction {type(value).__name__}; not registered")
    return None
