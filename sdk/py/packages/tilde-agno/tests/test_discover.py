"""`tilde deploy --dry-run` on a module-level Agno agent, through the installed `tilde.discover`
entry point, and `tilde_agno` stamping the same dynamic version at runtime."""

from __future__ import annotations

import json
import os
import sys
import textwrap
from types import SimpleNamespace
from unittest.mock import AsyncMock

import pytest

from tilde import deploy
from tilde._cancel import Cancellation
from tilde_agno import tilde_agno

AGENTS = """
from agno.agent import Agent
from agno.skills import LocalSkills
from agno.tools.function import Function

from tilde import BundledOptions, define_tools
from tilde_agno import TildeSkills


def lookup(order_id: str) -> str:
    '''Look up an order.'''
    return "shipped"


def ping() -> str:
    '''Check the service.'''
    return "pong"


TOOLS = define_tools([lookup], options={"lookup": BundledOptions(summary="Looked up")})


def instructions(run_context) -> str:
    return "Answer briefly."


responder = Agent(
    instructions=instructions,
    description="You help {user_id}.",
    system_message=None,
    skills=TildeSkills([LocalSkills("skills")]),
)
reviewer = Agent(
    name="Reviewer",
    instructions=["Check facts.", "Be kind."],
    tools=[
        Function(
            name="ping",
            entrypoint=ping,
            annotations={"readOnlyHint": True, "openWorldHint": True},
        ),
        {"type": "function", "function": {"name": "raw"}},
    ],
)
"""


@pytest.fixture
def project(tmp_path, monkeypatch):
    (tmp_path / "agno_agents.py").write_text(AGENTS)
    (tmp_path / "main.py").write_text("from agno_agents import responder, reviewer  # noqa: F401\n")
    skill = tmp_path / "skills" / "citations"
    skill.mkdir(parents=True)
    (skill / "SKILL.md").write_text(
        textwrap.dedent("""\
        ---
        name: citations
        description: Cite sources.
        ---
        Always cite.
        """)
    )
    monkeypatch.chdir(tmp_path)
    monkeypatch.syspath_prepend(str(tmp_path))
    before = set(sys.modules)
    yield tmp_path
    for name in set(sys.modules) - before:
        del sys.modules[name]
    os.environ.pop("TILDE_DISCOVERY", None)


async def test_agent_prompts_and_skills_are_declared_and_dynamic_ones_stamped(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    declarations = json.loads(capsys.readouterr().out)
    prompts = {
        p["name"]: (p["format"], p["template"], p["origin"], p["hash"])
        for p in declarations["prompts"]
    }
    dynamic = prompts.pop("responder/instructions")
    assert dynamic[0] == "PROMPT_FORMAT_DYNAMIC"
    assert dynamic[1].startswith("def instructions(run_context)")
    assert {name: value[:3] for name, value in prompts.items()} == {
        "responder/description": (
            "PROMPT_FORMAT_BRACES",
            "You help {user_id}.",
            "main.py#responder.description",
        ),
        "Reviewer/instructions/1": (
            "PROMPT_FORMAT_PLAIN",
            "Check facts.",
            "main.py#reviewer.instructions[0]",
        ),
        "Reviewer/instructions/2": (
            "PROMPT_FORMAT_PLAIN",
            "Be kind.",
            "main.py#reviewer.instructions[1]",
        ),
    }
    [skill] = declarations["skills"]
    assert (skill["name"], skill["origin"]) == ("citations", "skills/citations")

    # At runtime the unnamed agent is found under the same variable and stamped with that hash.
    stamps: dict[str, str] = {}
    empty = project / "registry"
    empty.mkdir()

    async def directory() -> str:
        return str(empty)

    ctx = SimpleNamespace(
        channel=SimpleNamespace(current={}),
        agent_tools={},
        skills=SimpleNamespace(directory=directory),
        cancellation=Cancellation(),
        thread_id="thread-1",
        activate_prompt=stamps.__setitem__,
        _set_bundled_tools=AsyncMock(),
    )
    await tilde_agno(ctx, sys.modules["agno_agents"].responder)  # type: ignore[arg-type]
    assert stamps == {"responder/instructions": dynamic[3]}


def test_bundled_plain_functions_and_agent_tools_are_declared_as_published(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    tools = {t["name"]: t for t in json.loads(capsys.readouterr().out)["tools"]}
    assert tools == {
        "lookup": {
            "name": "lookup",
            "description": "Look up an order.",
            "summary": "Looked up",
            "inputSchemaJson": json.dumps(
                {
                    "additionalProperties": False,
                    "properties": {"order_id": {"type": "string"}},
                    "required": ["order_id"],
                    "type": "object",
                },
                sort_keys=True,
            ),
            "display": "TOOL_DISPLAY_FULL",
            "origin": "agno_agents.py#TOOLS.tools.lookup",
        },
        # The dict tool is not something with_tilde_tools publishes.
        "ping": {
            "name": "ping",
            "description": "Check the service.",
            "inputSchemaJson": json.dumps(
                {"properties": {}, "required": [], "type": "object"}, sort_keys=True
            ),
            "annotations": {"readOnly": True, "openWorld": True},
            "display": "TOOL_DISPLAY_FULL",
            "origin": "main.py#reviewer.tools.ping",
        },
    }
