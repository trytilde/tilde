"""``tilde-declarations [ENTRY]``: print the prompts, skills and bundled tools an agent's code
declares, as protobuf JSON on stdout.

The entry module is imported with ``TILDE_DISCOVERY=1`` (hosts do not dial) under a name
other than ``__main__``. Its globals, and the globals of every module loaded from under the
project directory (cwd), are offered to Tilde's own ``define_*`` recognition and to every
framework discoverer registered in the ``tilde.discover`` entry point group (which also declare
``define_tools`` values).

Only this interpreter can read declarations out of an agent's own modules, so this is the half
of ``tilde deploy`` that has to live in the SDK. The ``tilde`` CLI runs it and owns everything
after: uploading skill files and registering the deployment, once, for every language.
"""

from __future__ import annotations

import argparse
import contextlib
import importlib
import importlib.metadata
import importlib.util
import os
import sys
import traceback
from collections.abc import Callable, Iterable
from pathlib import Path
from types import ModuleType
from typing import Any

from google.protobuf.json_format import MessageToJson

from tilde._tools import BundledTools
from tilde.discovery import (
    PROMPT_FORMAT_MUSTACHE,
    Discovered,
    DiscoveryContext,
    declared_prompt,
)
from tilde.management.v1 import deployments_pb2
from tilde.prompts import PromptDefinition
from tilde.skills import ENTRYPOINT, SkillDefinition, SkillsDefinition

MAX_INLINE_BYTES = 256 * 1024
Discoverer = Callable[[Any, DiscoveryContext], Discovered | None]
FORMATS = {1: "plain", 2: "mustache", 3: "braces", 4: "dynamic"}


class DeployError(Exception):
    pass


def load_entry(entry: str, root: Path) -> ModuleType:
    """Import a file path or dotted module without running its ``__main__`` block."""
    path = Path(entry)
    if entry.endswith(".py") or path.is_file():
        path = (root / path).resolve()
        if not path.is_file():
            raise DeployError(f"Entry {entry} does not exist")
        sys.path.insert(0, str(path.parent))
        name = path.stem if path.stem not in sys.modules else f"tilde_entry_{path.stem}"
        spec = importlib.util.spec_from_file_location(name, path)
        if spec is None or spec.loader is None:
            raise DeployError(f"Cannot load {entry}")
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
        return module
    sys.path.insert(0, str(root))
    return importlib.import_module(entry)


def project_modules(entry: ModuleType, root: Path) -> list[ModuleType]:
    """The entry module first, then every other module whose file lives in the project."""
    environments = {Path(sys.prefix).resolve(), Path(sys.base_prefix).resolve()}
    modules = [entry]
    for module in list(sys.modules.values()):
        file = getattr(module, "__file__", None)
        if module is entry or not file:
            continue
        path = Path(file).resolve()
        if not path.is_relative_to(root) or "site-packages" in path.parts:
            continue
        if any(path.is_relative_to(prefix) for prefix in environments):
            continue
        modules.append(module)
    return modules


def imported_packages(modules: list[ModuleType]) -> frozenset[str]:
    """Top-level packages the project's modules import modules, classes or functions from."""
    packages = set()
    for module in modules:
        for value in list(vars(module).values()):
            owner = (
                value.__name__
                if isinstance(value, ModuleType)
                else getattr(value, "__module__", None)
            )
            if isinstance(owner, str):
                packages.add(owner.partition(".")[0])
    return frozenset(packages)


def load_discoverers(warnings: list[str]) -> list[tuple[str, Discoverer]]:
    discoverers = []
    for point in importlib.metadata.entry_points(group="tilde.discover"):
        try:
            discoverers.append((point.name, point.load()))
        except Exception as error:  # noqa: BLE001 - an unusable adapter must not block deploys
            warnings.append(f"discoverer {point.name} could not be loaded: {error}")
    return discoverers


def _core(value: Any, context: DiscoveryContext) -> Discovered | None:
    if isinstance(value, PromptDefinition):
        file = value.file or Path(sys.modules[context.module].__file__ or "")
        origin = f"{context.relative(file)}#{value.name}"
        prompt = declared_prompt(
            value.name,
            value.template,
            PROMPT_FORMAT_MUSTACHE,
            origin,
            sections=value.sections,
            config=value.config_json,
        )
        return Discovered(prompts=[prompt])
    if isinstance(value, SkillsDefinition):
        return Discovered(skills=value.skills())
    if isinstance(value, SkillDefinition):
        return Discovered(skills=[value])
    return None


def discover(
    modules: Iterable[ModuleType],
    root: Path,
    discoverers: list[tuple[str, Discoverer]],
    warnings: list[str],
    imported: frozenset[str] = frozenset(),
) -> deployments_pb2.DeploymentDeclarations:
    """Scan module globals; identical declarations are merged, conflicting ones are an error."""
    prompts: dict[str, deployments_pb2.DeclaredPrompt] = {}
    skills: dict[str, deployments_pb2.DeclaredSkill] = {}
    tools: dict[str, deployments_pb2.DeclaredTool] = {}
    seen: set[int] = set()
    for module in modules:
        for name, value in list(vars(module).items()):
            if name.startswith("__") or isinstance(value, ModuleType) or id(value) in seen:
                continue
            seen.add(id(value))
            context = DiscoveryContext(root, module.__name__, name, warnings, imported)
            found = [_core(value, context)]
            for adapter, discoverer in discoverers:
                try:
                    found.append(discoverer(value, context))
                except Exception as error:
                    raise DeployError(
                        f"{adapter} could not read {module.__name__}.{name}: {error}"
                    ) from error
            if isinstance(value, BundledTools) and value.tools:
                if not any(result is not None and result.tools for result in found):
                    context.warn(
                        f"{context.origin()}: no installed adapter recognises these tools; "
                        "they are not declared"
                    )
            for result in found:
                if result is None:
                    continue
                for prompt in result.prompts:
                    _merge_prompt(prompts, prompt)
                for skill in result.skills:
                    _merge_skill(skills, _declared_skill(skill, context))
                for tool in result.tools:
                    _merge_tool(tools, tool)
    return deployments_pb2.DeploymentDeclarations(
        prompts=prompts.values(), skills=skills.values(), tools=tools.values()
    )


def _merge_prompt(
    into: dict[str, deployments_pb2.DeclaredPrompt], prompt: deployments_pb2.DeclaredPrompt
) -> None:
    existing = into.get(prompt.name)
    if existing is None:
        into[prompt.name] = prompt
    elif existing.hash != prompt.hash:
        raise DeployError(
            f"Prompt {prompt.name} is declared twice with different content "
            f"({existing.origin} and {prompt.origin})"
        )


def _merge_skill(
    into: dict[str, deployments_pb2.DeclaredSkill], skill: deployments_pb2.DeclaredSkill
) -> None:
    existing = into.get(skill.name)
    if existing is None:
        into[skill.name] = skill
    elif list(existing.files) != list(skill.files):
        raise DeployError(
            f"Skill {skill.name} is declared twice with different files "
            f"({existing.origin} and {skill.origin})"
        )


def _merge_tool(
    into: dict[str, deployments_pb2.DeclaredTool], tool: deployments_pb2.DeclaredTool
) -> None:
    existing = into.get(tool.name)
    if existing is None:
        into[tool.name] = tool
    elif _definition(existing) != _definition(tool):
        raise DeployError(
            f"Tool {tool.name} is declared twice with different definitions "
            f"({existing.origin} and {tool.origin})"
        )


def _definition(tool: deployments_pb2.DeclaredTool) -> deployments_pb2.DeclaredTool:
    copy = deployments_pb2.DeclaredTool()
    copy.CopyFrom(tool)
    copy.ClearField("origin")
    return copy


def _declared_skill(
    skill: SkillDefinition, context: DiscoveryContext
) -> deployments_pb2.DeclaredSkill:
    if skill.source is None:
        origin = f"{context.module}#{context.name}"
    elif skill.source.is_dir():
        origin = context.relative(skill.source)
    else:
        origin = f"{context.relative(skill.source)}#{skill.name}"
    files = []
    for file in sorted(skill.files, key=lambda f: f.path):
        declared = deployments_pb2.DeclaredSkillFile(path=file.path, executable=file.executable)
        # SKILL.md (capped at 1 MiB) always travels inline: the server reads its front matter.
        text = _text(file.data, inline=file.path == ENTRYPOINT)
        if text is not None:
            declared.content = text
        else:
            declared.data = file.data
        files.append(declared)
    return deployments_pb2.DeclaredSkill(name=skill.name, files=files, origin=origin)


def _text(data: bytes, inline: bool = False) -> str | None:
    if not inline and len(data) > MAX_INLINE_BYTES:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def inventory(declarations: deployments_pb2.DeploymentDeclarations, warnings: list[str]) -> str:
    prompts, skills, tools = declarations.prompts, declarations.skills, declarations.tools
    lines = [
        f"Discovered {len(prompts)} prompt(s), {len(skills)} skill(s) and {len(tools)} tool(s)"
    ]
    for prompt in prompts:
        lines.append(f"  prompt {prompt.name} ({FORMATS.get(prompt.format, '?')}) {prompt.origin}")
    for skill in skills:
        lines.append(f"  skill  {skill.name} ({len(skill.files)} files) {skill.origin}")
    for tool in tools:
        lines.append(f"  tool   {tool.name} {tool.origin}")
    lines.extend(f"  warning: {warning}" for warning in warnings)
    return "\n".join(lines)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="tilde-declarations",
        description=(
            "Print the prompts, skills and tools an agent declares, as protobuf JSON. "
            "Run `tilde deploy` to register a deployment carrying them."
        ),
    )
    parser.add_argument("entry", nargs="?", default="main.py", help="file path or dotted module")
    parser.add_argument(
        "--allow-empty",
        action="store_true",
        help="print empty declarations instead of failing, for an agent that declares nothing",
    )
    return parser.parse_args(argv)


def run(argv: list[str]) -> int:
    args = parse_args(argv)
    os.environ["TILDE_DISCOVERY"] = "1"
    root = Path.cwd().resolve()
    warnings: list[str] = []
    # Stdout carries only the JSON the `tilde` CLI reads; agent imports may print.
    with contextlib.redirect_stdout(sys.stderr):
        # The entry first: an adapter importing its framework must not shadow a project module
        # of the same name (a project `agents.py` beside the OpenAI Agents SDK's `agents`).
        try:
            entry = load_entry(args.entry, root)
        except DeployError:
            raise
        except BaseException as error:
            traceback.print_exc()
            raise DeployError(f"Importing {args.entry} failed: {error}") from error
        discoverers = load_discoverers(warnings)
        modules = project_modules(entry, root)
        declarations = discover(modules, root, discoverers, warnings, imported_packages(modules))
    print(inventory(declarations, warnings), file=sys.stderr)
    if (
        not args.allow_empty
        and not declarations.prompts
        and not declarations.skills
        and not declarations.tools
    ):
        raise DeployError(f"Nothing to deploy: {args.entry} declares no prompts, skills or tools")
    print(MessageToJson(declarations))
    return 0


def main(argv: list[str] | None = None) -> int:
    try:
        return run(sys.argv[1:] if argv is None else argv)
    except DeployError as error:
        print(f"tilde-declarations: {error}", file=sys.stderr)
        return 1
