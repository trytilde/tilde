"""Prompts declared in code with ``define_prompt``, registered when ``tilde deploy`` runs.

The content hash follows ``crates/tilde/src/prompts/mod.rs`` (``content_hash``) and the TS
SDK's ``promptHash``: template, sections sorted by name, then the config as canonical JSON
(sorted keys, compact separators, as JavaScript's ``JSON.stringify`` writes it).
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from tilde._invocation import current

_NAME = re.compile(r"^[A-Za-z0-9._/-]{1,128}$")
_PLACEHOLDER = re.compile(r"\{\{\s*(>\s*)?([A-Za-z0-9_./-]+)\s*\}\}")
_VARIABLE = re.compile(r"^[A-Za-z0-9_]+$")


def _js_numbers(value: Any) -> Any:
    # JSON.stringify writes integral numbers without a fraction (2.0 -> 2).
    if isinstance(value, float) and value.is_integer() and abs(value) < 1e21:
        return int(value)
    if isinstance(value, dict):
        return {key: _js_numbers(inner) for key, inner in value.items()}
    if isinstance(value, list | tuple):
        return [_js_numbers(inner) for inner in value]
    return value


def canonical_json(value: Any) -> str:
    """JSON with object keys sorted at every level, so equal configs hash equally."""
    return json.dumps(_js_numbers(value), sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def prompt_hash(template: str, sections: Mapping[str, str], config: str) -> str:
    """Lowercase hex of the engine's prompt content hash."""
    digest = hashlib.sha256()
    digest.update(b"template\0" + template.encode() + b"\0")
    for name in sorted(sections):
        digest.update(b"section\0" + name.encode() + b"\0" + sections[name].encode() + b"\0")
    digest.update(b"config\0" + config.encode())
    return digest.hexdigest()


def valid_prompt_name(name: str) -> bool:
    return bool(_NAME.match(name))


class PromptDefinition:
    """A mustache prompt (``{{var}}``, ``{{> section}}``) declared at module scope."""

    def __init__(
        self,
        name: str,
        template: str,
        sections: Mapping[str, str] | None,
        config: Mapping[str, Any] | None,
        file: Path | None = None,
    ) -> None:
        if not valid_prompt_name(name):
            raise ValueError("Prompt names use letters, digits, '.', '_', '-' or '/'")
        if not template:
            raise ValueError(f"Prompt {name} has an empty template")
        self.name = name
        self.template = template
        self.sections = dict(sections or {})
        self.config = dict(config or {})
        # The defining source file; `tilde deploy` reports `<file>#<name>` as the origin.
        self.file = file
        for match in _PLACEHOLDER.finditer(template):
            if match[1] and match[2] not in self.sections:
                section = "{{> " + match[2] + "}}"
                raise ValueError(f"Prompt {name} includes an undeclared section {section}")
        self.config_json = canonical_json(self.config)
        self.hash = prompt_hash(self.template, self.sections, self.config_json)

    @property
    def stamp(self) -> str:
        """``name@hash``, stamped in ``x-tilde-prompt`` on inference calls."""
        return f"{self.name}@{self.hash}"

    def render(self, **variables: str | int | float | bool) -> str:
        """Inline sections, then substitute every ``{{variable}}``; a missing one is an error.

        Inside an invocation a successful render marks this prompt active, so later inference
        calls in the invocation carry its stamp.
        """

        def substitute(key: str, whole: str) -> str:
            if not _VARIABLE.match(key):
                return whole
            if key not in variables:
                raise KeyError(f"Prompt variable {key} was not supplied")
            value = variables[key]
            return str(value).lower() if isinstance(value, bool) else str(value)

        def replace(match: re.Match[str]) -> str:
            if match[1]:
                return _PLACEHOLDER.sub(
                    lambda inner: inner[0] if inner[1] else substitute(inner[2], inner[0]),
                    self.sections[match[2]],
                )
            return substitute(match[2], match[0])

        rendered = _PLACEHOLDER.sub(replace, self.template)
        ctx = current.get()
        if ctx is not None:
            ctx.prompt(self)
        return rendered


def caller_file(depth: int = 2) -> Path:
    return Path(sys._getframe(depth).f_code.co_filename).resolve()


def define_prompt(
    name: str,
    *,
    template: str,
    sections: Mapping[str, str] | None = None,
    config: Mapping[str, Any] | None = None,
) -> PromptDefinition:
    """Declare a prompt at module scope; ``tilde deploy`` registers it with the deployment."""
    return PromptDefinition(name, template, sections, config, caller_file())
