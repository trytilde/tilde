"""Skills shipped with a deployment: ``define_skills(path)`` for a directory of skill folders,
``define_skill(...)`` for one written in code. ``tilde deploy`` registers them. At runtime
``ctx.skills`` (``SkillsClient``) reads every skill the invocation may use, including those
assigned through the registry after the deployment shipped.

Package rules follow ``crates/tilde/src/skills/package.rs``: a ``SKILL.md`` with front matter
``name``/``description``, at most 2048 files, 10 MiB per file (1 MiB for ``SKILL.md``) and
64 MiB in total, relative paths without symlinks. ``.git`` and ``__pycache__`` are skipped.
"""

from __future__ import annotations

import asyncio
import json
import os
import re
import shutil
import tempfile
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import httpx

from tilde._tools import Tool, ToolCatalog, as_object, as_string
from tilde.prompts import caller_file
from tilde.runtime.v1.skills_connect import SkillServiceClient
from tilde.runtime.v1.skills_pb2 import ListSkillsRequest, ReadSkillFileRequest, SkillSummary

ENTRYPOINT = "SKILL.md"
MAX_FILES = 2048
MAX_FILE_BYTES = 10 * 1024 * 1024
MAX_ENTRYPOINT_BYTES = 1024 * 1024
MAX_TOTAL_BYTES = 64 * 1024 * 1024
_SKIPPED = {".git", "__pycache__"}
_NAME = re.compile(r"^[a-z0-9][a-z0-9_-]{0,63}$")


@dataclass(slots=True)
class SkillFile:
    path: str
    data: bytes
    executable: bool = False


@dataclass(slots=True)
class SkillDefinition:
    """One skill: its files (``SKILL.md`` included) and where it was declared."""

    name: str
    files: list[SkillFile]
    # Absolute directory or defining source file; `tilde deploy` reports it relative to cwd.
    source: Path | None = None

    def __post_init__(self) -> None:
        if not _NAME.match(self.name):
            raise ValueError(
                f"Skill name {self.name} must use lowercase letters, digits, '-' or '_' "
                "(1-64 characters)"
            )
        paths = [file.path for file in self.files]
        if ENTRYPOINT not in paths:
            raise ValueError(f"Skill {self.name} needs a {ENTRYPOINT}")
        if len(self.files) > MAX_FILES:
            raise ValueError(f"Skill {self.name} has more than {MAX_FILES} files")
        if len(set(paths)) != len(paths):
            raise ValueError(f"Skill {self.name} repeats a file path")
        total = 0
        for file in self.files:
            if not _safe_path(file.path):
                raise ValueError(f"Skill {self.name} has an unsafe path {file.path}")
            cap = MAX_ENTRYPOINT_BYTES if file.path == ENTRYPOINT else MAX_FILE_BYTES
            if len(file.data) > cap:
                raise ValueError(f"{file.path} in skill {self.name} is larger than the limit")
            total += len(file.data)
        if total > MAX_TOTAL_BYTES:
            raise ValueError(f"Skill {self.name} is larger than 64 MiB")


@dataclass(slots=True)
class SkillsDefinition:
    """A directory whose subfolders each hold a ``SKILL.md``; pass ``path`` to frameworks."""

    path: Path
    _loaded: list[SkillDefinition] | None = field(default=None, repr=False)

    def skills(self) -> list[SkillDefinition]:
        if self._loaded is None:
            self._loaded = read_skills(self.path)
        return self._loaded


def _safe_path(path: str) -> bool:
    return 0 < len(path) <= 512 and all(
        segment not in ("", ".", "..") and "\\" not in segment and segment.isprintable()
        for segment in path.split("/")
    )


def _front_matter_name(skill_md: str) -> str | None:
    lines = skill_md.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    for line in lines[1:]:
        if line.strip() == "---":
            break
        if line.startswith("name:"):
            return line[5:].strip().strip("\"'") or None
    return None


def _normalize(value: str) -> str:
    name = re.sub(r"[^a-z0-9]+", "-", value.strip().lower()).strip("-")
    return name[:64].rstrip("-")


def read_skill(directory: Path) -> SkillDefinition:
    """Read one skill folder; its name comes from the front matter, else the folder name."""
    directory = directory.resolve()
    files: list[SkillFile] = []
    for root, dirs, names in os.walk(directory):
        dirs[:] = sorted(d for d in dirs if d not in _SKIPPED)
        for name in sorted(names):
            path = Path(root) / name
            if path.is_symlink():
                raise ValueError(f"Skill files cannot be symlinks: {path}")
            files.append(
                SkillFile(
                    path=path.relative_to(directory).as_posix(),
                    data=path.read_bytes(),
                    executable=bool(path.stat().st_mode & 0o111),
                )
            )
    entry = next((f for f in files if f.path == ENTRYPOINT), None)
    if entry is None:
        raise ValueError(f"{directory} has no {ENTRYPOINT}")
    name = _front_matter_name(entry.data.decode("utf-8", "replace")) or _normalize(directory.name)
    return SkillDefinition(name, files, directory)


def read_skills(directory: Path) -> list[SkillDefinition]:
    """Every subfolder of ``directory`` holding a ``SKILL.md``."""
    if not directory.is_dir():
        raise ValueError(f"Skills directory {directory} does not exist")
    return [
        read_skill(child)
        for child in sorted(directory.iterdir())
        if child.is_dir() and (child / ENTRYPOINT).is_file()
    ]


def define_skills(path: str | os.PathLike[str]) -> SkillsDefinition:
    """Declare a directory of skill folders; relative paths resolve from the calling file."""
    resolved = Path(path)
    if not resolved.is_absolute():
        resolved = caller_file().parent / resolved
    return SkillsDefinition(resolved.resolve())


def define_skill(
    name: str,
    description: str,
    instructions: str,
    files: dict[str, str | bytes] | None = None,
) -> SkillDefinition:
    """Declare a skill in code; its ``SKILL.md`` is generated with front matter."""
    if "\n" in description or not description.strip():
        raise ValueError("Skill descriptions are one non-empty line")
    skill_md = f"---\nname: {name}\ndescription: {description}\n---\n\n{instructions.strip()}\n"
    extra = [
        SkillFile(path, body.encode() if isinstance(body, str) else body)
        for path, body in (files or {}).items()
    ]
    return SkillDefinition(name, [SkillFile(ENTRYPOINT, skill_md.encode()), *extra], caller_file())


@dataclass(slots=True)
class SkillFileBody:
    """Text arrives inline as ``content``; other files as a short-lived ``download_url``."""

    media_type: str
    content: str | None = None
    download_url: str | None = None


class SkillsClient:
    """``ctx.skills``: the skills this invocation may use, read through the runtime SkillService.

    ``list`` returns the deployment's own skills (``deployed``, files already beside the code)
    and those assigned through the registry (``deployed`` false) at their latest version.
    ``tools()`` returns ``list_skills`` and ``read_skill`` for frameworks without native skills;
    convert them like channel tools (``convert_to_<framework>_tools(ctx.skills.tools())``).
    """

    def __init__(
        self,
        client: SkillServiceClient,
        options: Callable[[], dict[str, Any]],
        agent_id: str = "",
        pushed: list[SkillSummary] | None = None,
    ) -> None:
        self._client = client
        self._options = options
        self._agent_id = agent_id
        # The skills the engine pushed with the wake; older engines send none and we list.
        self._pushed = pushed

    async def list(self, agent_id: str = "") -> list[SkillSummary]:
        if not agent_id and self._pushed is not None:
            return list(self._pushed)
        response = await self._client.list_skills(
            ListSkillsRequest(agent_id=agent_id), **self._options()
        )
        return list(response.skills)

    async def read(self, name: str, path: str = ENTRYPOINT, agent_id: str = "") -> SkillFileBody:
        """``name`` or ``source/name`` when two sources share a name."""
        file = await self._client.read_skill_file(
            ReadSkillFileRequest(name=name, path=path, agent_id=agent_id), **self._options()
        )
        if file.download_url:
            return SkillFileBody(file.media_type, download_url=file.download_url)
        return SkillFileBody(file.media_type, content=file.content)

    async def summary(self) -> str:
        """A system-prompt block naming every skill, for use with the skill tools."""
        skills = await self.list()
        if not skills:
            return ""
        return "\n".join(
            [
                "You have these skills. Read a skill's SKILL.md with the read_skill tool "
                "before using it.",
                *(f"- {_address(skill, skills)}: {skill.description}" for skill in skills),
            ]
        )

    async def directory(self) -> str:
        """The registry skills (``deployed`` false) in one folder per skill, and their root.

        The root is ``$TILDE_SKILLS_DIR`` (the OS temp directory by default, which lasts as long
        as a container) ``/tilde-skills/<agent id>``, with folders named by skill (or
        ``<source>-<name>`` when names collide). The version each folder holds is kept in
        memory, so an unchanged invocation touches no files; only new and newer versions are
        downloaded and removed skills deleted. Pass the root to frameworks that load skill
        folders from disk.
        """
        skills = [skill for skill in await self.list() if not skill.deployed]
        held = _local(self._agent_id)
        async with held.lock:
            wanted = {_address(skill, skills).replace("/", "-"): skill for skill in skills}
            held.root.mkdir(parents=True, exist_ok=True)
            changed = False
            async with httpx.AsyncClient(timeout=60.0) as downloads:
                for folder, skill in wanted.items():
                    if held.versions.get(folder) == skill.version_id:
                        continue
                    # Written beside the target and swapped in, so a reader never sees half a skill.
                    staging = Path(tempfile.mkdtemp(prefix=f".{folder}-", dir=held.root))
                    try:
                        await self._write(skill, skills, staging, downloads)
                        shutil.rmtree(held.root / folder, ignore_errors=True)
                        os.rename(staging / folder, held.root / folder)
                    finally:
                        shutil.rmtree(staging, ignore_errors=True)
                    held.versions[folder] = skill.version_id
                    changed = True
            for folder in [f for f in held.versions if f not in wanted]:
                shutil.rmtree(held.root / folder, ignore_errors=True)
                del held.versions[folder]
                changed = True
            if changed:
                (held.root / ".versions.json").write_text(json.dumps(held.versions))
        return str(held.root)

    async def _write(
        self,
        skill: SkillSummary,
        skills: list[SkillSummary],
        staging: Path,
        downloads: httpx.AsyncClient,
    ) -> None:
        address = _address(skill, skills)
        folder = address.replace("/", "-")
        for info in skill.files:
            if not _safe_path(info.path) or not _safe_path(folder):
                raise ValueError(f"Skill {address} has an unsafe path {info.path}")
            target = staging / folder / info.path
            target.parent.mkdir(parents=True, exist_ok=True)
            body = await self.read(address, info.path)
            if body.content is not None:
                target.write_text(body.content, encoding="utf-8")
            else:
                # Presigned: the URL is the credential, so no authorization header is sent.
                response = await downloads.get(body.download_url or "")
                if response.status_code != 200:
                    raise RuntimeError(f"Unable to download {address}/{info.path}")
                target.write_bytes(response.content)
            if info.executable:
                target.chmod(0o755)

    def tools(self) -> ToolCatalog:
        """``list_skills`` and ``read_skill``, the fallback for frameworks without native skills."""

        async def list_skills(_input: Any, _call: str) -> Any:
            skills = await self.list()
            return [
                {
                    "name": _address(skill, skills),
                    "description": skill.description,
                    "files": [file.path for file in skill.files],
                }
                for skill in skills
            ]

        async def read_skill(input: Any, _call: str) -> Any:
            fields = as_object(input)
            path = fields.get("path") or ENTRYPOINT
            if not isinstance(path, str):
                raise ValueError("path must be a string")
            body = await self.read(as_string(fields, "name"), path)
            if body.content is not None:
                return body.content
            return {"mediaType": body.media_type, "downloadUrl": body.download_url}

        string = {"type": "string"}
        return {
            "list_skills": Tool(
                description="List your skills: name, description and files.",
                input_schema={"type": "object", "properties": {}, "additionalProperties": False},
                _execute=list_skills,
            ),
            "read_skill": Tool(
                description=(
                    "Read a file of one of your skills, SKILL.md by default. Read a skill's "
                    "SKILL.md before using it; binary files return a short-lived download URL."
                ),
                input_schema={
                    "type": "object",
                    "properties": {
                        "name": {**string, "description": "Skill name as listed"},
                        "path": {**string, "description": "File path within the skill"},
                    },
                    "required": ["name"],
                    "additionalProperties": False,
                },
                _execute=read_skill,
            ),
        }


def _address(skill: SkillSummary, skills: list[SkillSummary]) -> str:
    # Two sources may share a skill name; only those are qualified.
    repeated = sum(1 for other in skills if other.name == skill.name) > 1
    return f"{skill.source}/{skill.name}" if repeated else skill.name


@dataclass(slots=True)
class _Local:
    """Registry skills this process holds for one agent: its folder and each skill's version."""

    root: Path
    versions: dict[str, str]
    lock: asyncio.Lock = field(default_factory=asyncio.Lock)


_LOCALS: dict[str, _Local] = {}


def _local(agent_id: str) -> _Local:
    held = _LOCALS.get(agent_id)
    if held is None:
        base = os.environ.get("TILDE_SKILLS_DIR") or tempfile.gettempdir()
        root = Path(base) / "tilde-skills" / (agent_id or "agent")
        versions: dict[str, str] = {}
        try:
            saved = json.loads((root / ".versions.json").read_text())
            versions = {f: v for f, v in saved.items() if (root / f).is_dir()}
        except (OSError, ValueError):
            pass  # first run, or an unreadable manifest: every skill downloads again
        held = _LOCALS.setdefault(agent_id, _Local(root, versions))
    return held
