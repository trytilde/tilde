"""`tilde deploy --dry-run` on a @CrewBase project, through the installed `tilde.discover`
entry point: raw YAML fields become prompts, YAML skill paths become skills."""

from __future__ import annotations

import json
import os
import sys
import textwrap

import pytest

from tilde import deploy

CREW = """
from crewai.project import CrewBase


@CrewBase
class ResearchCrew:
    agents_config = "config/agents.yaml"
    tasks_config = "config/tasks.yaml"
"""
AGENTS = """
researcher:
  role: >
    {topic} Senior Data Researcher
  goal: Uncover cutting-edge developments
  backstory: You're a seasoned researcher.
  llm: openai/gpt-4o-mini
  skills:
    - skills
"""
TASKS = """
research_task:
  description: Conduct a thorough research about {topic}
  expected_output: A list with 10 bullet points
  agent: researcher
"""


@pytest.fixture
def project(tmp_path, monkeypatch):
    package = tmp_path / "research"
    (package / "config").mkdir(parents=True)
    (package / "__init__.py").write_text("")
    # The crew lives in a project module the entry imports; CrewAI resolves its YAML from there.
    (package / "crew.py").write_text(CREW)
    (package / "config" / "agents.yaml").write_text(AGENTS)
    (package / "config" / "tasks.yaml").write_text(TASKS)
    (tmp_path / "main.py").write_text("from research.crew import ResearchCrew  # noqa: F401\n")
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


def test_crewbase_yaml_prompts_and_skills_are_declared(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    out, err = capsys.readouterr()
    declarations = json.loads(out)
    prompts = {
        p["name"]: (p["format"], p["template"], p["origin"]) for p in declarations["prompts"]
    }
    assert prompts == {
        # Raw YAML, before CrewAI interpolates {topic}.
        "agents/researcher/role": (
            "PROMPT_FORMAT_BRACES",
            "{topic} Senior Data Researcher\n",
            "research/config/agents.yaml#researcher.role",
        ),
        "agents/researcher/goal": (
            "PROMPT_FORMAT_PLAIN",
            "Uncover cutting-edge developments",
            "research/config/agents.yaml#researcher.goal",
        ),
        "agents/researcher/backstory": (
            "PROMPT_FORMAT_PLAIN",
            "You're a seasoned researcher.",
            "research/config/agents.yaml#researcher.backstory",
        ),
        "tasks/research_task/description": (
            "PROMPT_FORMAT_BRACES",
            "Conduct a thorough research about {topic}",
            "research/config/tasks.yaml#research_task.description",
        ),
        "tasks/research_task/expected_output": (
            "PROMPT_FORMAT_PLAIN",
            "A list with 10 bullet points",
            "research/config/tasks.yaml#research_task.expected_output",
        ),
    }
    [skill] = declarations["skills"]
    assert (skill["name"], skill["origin"]) == ("citations", "skills/citations")
    assert "prompt tasks/research_task/description (braces)" in err


TOOLS_MAIN = """
from crewai.tools import tool

from tilde import BundledOptions, define_tools


@tool("Lookup Order")
def lookup(order_id: str) -> str:
    \"\"\"Look up an order.\"\"\"
    return "shipped"


TOOLS = define_tools([lookup], options={"Lookup Order": BundledOptions(summary="Looked up")})
"""


def test_bundled_tools_are_declared_under_the_name_crewai_shows(tmp_path, monkeypatch, capsys):
    (tmp_path / "tools_main.py").write_text(TOOLS_MAIN)
    monkeypatch.chdir(tmp_path)
    monkeypatch.syspath_prepend(str(tmp_path))
    assert deploy.main(["tools_main.py", "--dry-run"]) == 0
    (declared,) = json.loads(capsys.readouterr().out)["tools"]
    # CrewAI shows the model a sanitized name; options stay keyed by the tool's own name.
    assert declared["name"] == "lookup_order"
    assert declared["summary"] == "Looked up"
    assert json.loads(declared["inputSchemaJson"])["properties"]["order_id"]["type"] == "string"
