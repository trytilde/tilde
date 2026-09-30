"""`tilde deploy --dry-run` on an OpenAI Agents SDK project, through the installed
`tilde.discover` entry point: agents reached through handoffs and agent tools (cycles
included), dynamic instructions, hosted prompts and local skill folders."""

from __future__ import annotations

import json
import os
import sys
import textwrap

import pytest

from tilde import deploy

MAIN = """
from agents import Agent, ShellTool, function_tool, handoff
from agents.sandbox import SandboxAgent
from agents.sandbox.capabilities import Skills
from agents.sandbox.entries import LocalDir

from tilde import BundledOptions, ToolAnnotations, define_tools


@function_tool
def lookup(order_id: str) -> str:
    '''Look up an order.'''
    return "shipped"


@function_tool
def refund(order_id: str) -> str:
    '''Refund an order.'''
    return "refunded"


TOOLS = define_tools(
    [lookup],
    options={
        "lookup": BundledOptions(
            summary="Looked up", display="hidden", annotations=ToolAnnotations(idempotent=True)
        )
    },
)


def billing_instructions(context, agent):
    return f"Help {context.context} with billing."


billing = Agent(
    name="Billing agent",
    instructions=billing_instructions,
    handoff_description="Billing questions",
)
researcher = SandboxAgent(
    name="researcher",
    instructions="Research.",
    capabilities=[Skills(from_=LocalDir(src="skills"))],
)
hosted = Agent(name="hosted", prompt={"id": "pmpt_123", "version": "2"})
triage = Agent(
    name="triage",
    instructions="Route the question.",
    handoffs=[handoff(billing), hosted],
    tools=[
        refund,
        researcher.as_tool(tool_name="research", tool_description="Research a topic"),
        ShellTool(
            executor=lambda request: "",
            environment={
                "type": "local",
                "skills": [
                    {"name": "citations", "description": "Cite", "path": "skills/citations"}
                ],
            },
        ),
    ],
)
billing.handoffs.append(triage)
"""


@pytest.fixture
def project(tmp_path, monkeypatch):
    (tmp_path / "main.py").write_text(MAIN)
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


def test_reachable_agents_prompts_and_skills_are_declared(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    out, err = capsys.readouterr()
    declarations = json.loads(out)
    prompts = {p["name"]: p for p in declarations["prompts"]}
    # Globals are read in order: `billing` first, reaching `triage` through its handoff (and
    # `billing` again through triage's, which ends the cycle).
    assert {name: (p["format"], p["origin"]) for name, p in prompts.items()} == {
        "Billing-agent/instructions": ("PROMPT_FORMAT_DYNAMIC", "main.py#billing.instructions"),
        "Billing-agent/handoff_description": (
            "PROMPT_FORMAT_PLAIN",
            "main.py#billing.handoff_description",
        ),
        "triage/instructions": (
            "PROMPT_FORMAT_PLAIN",
            "main.py#billing.handoffs.triage.instructions",
        ),
        "researcher/instructions": (
            "PROMPT_FORMAT_PLAIN",
            "main.py#billing.handoffs.triage.tools.research.instructions",
        ),
    }
    assert prompts["Billing-agent/instructions"]["template"].startswith(
        "def billing_instructions(context, agent):"
    )
    [skill] = declarations["skills"]
    assert skill["name"] == "citations"
    assert "OpenAI-hosted prompt pmpt_123 is versioned by OpenAI" in err


def test_bundled_and_agent_function_tools_are_declared_as_published(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    tools = {t["name"]: t for t in json.loads(capsys.readouterr().out)["tools"]}
    assert {name: tool["origin"] for name, tool in tools.items()} == {
        "lookup": "main.py#TOOLS.tools.lookup",
        "refund": "main.py#billing.handoffs.triage.tools.refund",
        # An agent tool is a FunctionTool too; the shell tool is not declared.
        "research": "main.py#billing.handoffs.triage.tools.research",
    }
    lookup = tools["lookup"]
    assert json.loads(lookup.pop("inputSchemaJson")) == {
        "additionalProperties": False,
        "properties": {"order_id": {"title": "Order Id", "type": "string"}},
        "required": ["order_id"],
        "title": "lookup_args",
        "type": "object",
    }
    assert lookup == {
        "name": "lookup",
        "description": "Look up an order.",
        "summary": "Looked up",
        "annotations": {"idempotent": True},
        "display": "TOOL_DISPLAY_HIDDEN",
        "origin": "main.py#TOOLS.tools.lookup",
    }
    assert "summary" not in tools["refund"]
