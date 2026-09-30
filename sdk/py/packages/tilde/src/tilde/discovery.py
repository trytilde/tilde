"""What ``tilde deploy`` finds in an agent's modules, and the contract for framework adapters.

An adapter registers an entry point in the ``tilde.discover`` group naming a function
``discover(value, context) -> Discovered | None``. It is called with every global of the entry
module and of the project's own modules; it returns ``None`` for values it does not recognise
and must not mutate or call into them beyond reading declarations. A ``define_tools`` value
(``BundledTools``) is offered too: the adapter whose framework its tools belong to declares them.
"""

from __future__ import annotations

import json
import os
import sys
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path

from tilde._tools import ToolSpec
from tilde.management.v1.deployments_pb2 import DeclaredPrompt, DeclaredTool
from tilde.prompts import canonical_json, prompt_hash, valid_prompt_name
from tilde.skills import SkillDefinition
from tilde.types.v1.chat_pb2 import ToolAnnotations
from tilde.types.v1.prompt_pb2 import (
    PROMPT_FORMAT_BRACES,
    PROMPT_FORMAT_DYNAMIC,
    PROMPT_FORMAT_MUSTACHE,
    PROMPT_FORMAT_PLAIN,
    PromptFormat,
)
from tilde.types.v1.prompt_pb2 import PromptSection as _Section

__all__ = [
    "PROMPT_FORMAT_BRACES",
    "PROMPT_FORMAT_DYNAMIC",
    "PROMPT_FORMAT_MUSTACHE",
    "PROMPT_FORMAT_PLAIN",
    "Discovered",
    "DiscoveryContext",
    "declared_prompt",
    "declared_tool",
]


@dataclass(slots=True)
class DiscoveryContext:
    """Where a value was found. ``root`` is the project directory (the deploy's cwd);
    ``imported`` the top-level packages the project imports from, which tells whose plain
    functions a ``define_tools`` holds."""

    root: Path
    module: str
    name: str
    warnings: list[str] = field(default_factory=list)
    imported: frozenset[str] = frozenset()

    def warn(self, message: str) -> None:
        """Report something declared that cannot be registered, such as an unreadable prompt."""
        self.warnings.append(message)

    def relative(self, path: Path) -> str:
        return Path(os.path.relpath(path.resolve(), self.root)).as_posix()

    def origin(self) -> str:
        """``<file>#<variable>``, relative to the project."""
        file = getattr(sys.modules.get(self.module), "__file__", None)
        return f"{self.relative(Path(file)) if file else self.module}#{self.name}"


@dataclass(slots=True)
class Discovered:
    prompts: list[DeclaredPrompt] = field(default_factory=list)
    skills: list[SkillDefinition] = field(default_factory=list)
    tools: list[DeclaredTool] = field(default_factory=list)


def declared_prompt(
    name: str,
    template: str,
    format: PromptFormat,
    origin: str,
    *,
    sections: Mapping[str, str] | None = None,
    config: Mapping[str, object] | str | None = None,
) -> DeclaredPrompt:
    """A prompt as ``RegisterDeployment`` takes it, hashed like the engine hashes it."""
    if not valid_prompt_name(name):
        raise ValueError(f"Prompt name {name!r} must match [A-Za-z0-9._/-]{{1,128}}")
    sections = dict(sections or {})
    config_json = config if isinstance(config, str) else canonical_json(dict(config or {}))
    return DeclaredPrompt(
        name=name,
        format=format,
        template=template,
        sections=[_Section(name=key, content=sections[key]) for key in sorted(sections)],
        config=config_json,
        hash=prompt_hash(template, sections, config_json),
        origin=origin,
    )


def declared_tool(spec: ToolSpec, origin: str) -> DeclaredTool:
    """A bundled tool as ``RegisterDeployment`` takes it, from the adapter's ``describe_tool``."""
    if not 0 < len(spec.name) <= 128 or spec.name.startswith("tools."):
        raise ValueError(f"Tool name {spec.name!r} must be 1-128 characters, not tools.*")
    if not spec.description:
        raise ValueError(f"Tool {spec.name} has no description")
    hints = spec.annotations
    return DeclaredTool(
        name=spec.name,
        description=spec.description,
        summary=spec.summary or "",
        input_schema_json=json.dumps(spec.input_schema, sort_keys=True),
        output_schema_json=json.dumps(spec.output_schema, sort_keys=True)
        if spec.output_schema
        else "",
        annotations=ToolAnnotations(
            read_only=hints.read_only,
            destructive=hints.destructive,
            idempotent=hints.idempotent,
            open_world=hints.open_world,
        )
        if hints
        else None,
        display=f"TOOL_DISPLAY_{spec.display.upper()}",
        origin=origin,
    )
