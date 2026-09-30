"""`tilde deploy --dry-run` on a module-level Pydantic AI agent, through the installed
`tilde.discover` entry point, and the run capability stamping the same dynamic version."""

from __future__ import annotations

import json
import os
import sys
from types import SimpleNamespace

import pytest
from pydantic_ai.messages import ModelResponse, TextPart
from pydantic_ai.models.function import FunctionModel
from pydantic_ai.toolsets import FunctionToolset

from tilde import deploy
from tilde_pydantic_ai import TildeInvocation

AGENTS = """
from pydantic_ai import Agent, RunContext, Tool

from tilde import BundledOptions, ToolAnnotations, define_tools


def lookup(order_id: str) -> dict[str, str]:
    '''Look up an order.'''
    return {"status": "shipped"}


def ping() -> str:
    '''Check the service.'''
    return "pong"


TOOLS = define_tools(
    [Tool(lookup, metadata={"tilde": BundledOptions(summary="Looked up an order")})],
    options={
        "lookup": BundledOptions(
            summary="Looked up", display="summary", annotations=ToolAnnotations(read_only=True)
        )
    },
)
responder = Agent(
    instructions="You answer questions.", system_prompt=["Be kind.", "Be brief."], tools=[ping]
)


@responder.instructions
def today(ctx: RunContext) -> str:
    return "Today is Monday."
"""


@pytest.fixture
def project(tmp_path, monkeypatch):
    (tmp_path / "pydantic_agents.py").write_text(AGENTS)
    (tmp_path / "main.py").write_text("import pydantic_agents  # noqa: F401\n")
    monkeypatch.chdir(tmp_path)
    monkeypatch.syspath_prepend(str(tmp_path))
    before = set(sys.modules)
    yield tmp_path
    for name in set(sys.modules) - before:
        del sys.modules[name]
    os.environ.pop("TILDE_DISCOVERY", None)


async def test_agent_prompts_are_declared_and_dynamic_ones_stamped(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    prompts = {
        p["name"]: (p["format"], p["template"], p["origin"], p["hash"])
        for p in json.loads(capsys.readouterr().out)["prompts"]
    }
    dynamic = prompts.pop("responder/instructions/2")
    assert dynamic[:1] == ("PROMPT_FORMAT_DYNAMIC",)
    assert dynamic[1].startswith("@responder.instructions\ndef today(")
    assert {name: value[:3] for name, value in prompts.items()} == {
        "responder/instructions/1": (
            "PROMPT_FORMAT_PLAIN",
            "You answer questions.",
            "pydantic_agents.py#responder.instructions[0]",
        ),
        "responder/system_prompt/1": (
            "PROMPT_FORMAT_PLAIN",
            "Be kind.",
            "pydantic_agents.py#responder.system_prompt[0]",
        ),
        "responder/system_prompt/2": (
            "PROMPT_FORMAT_PLAIN",
            "Be brief.",
            "pydantic_agents.py#responder.system_prompt[1]",
        ),
    }

    responder = sys.modules["pydantic_agents"].responder
    stamps: dict[str, str] = {}
    ctx = SimpleNamespace(activate_prompt=stamps.__setitem__, take_inputs=list)
    capability = TildeInvocation(ctx, FunctionToolset([]))  # type: ignore[arg-type]
    model = FunctionModel(lambda _messages, _info: ModelResponse(parts=[TextPart("ok")]))
    await responder.run("hi", model=model, capabilities=[capability])
    assert stamps == {"responder/instructions/2": dynamic[3]}


def test_bundled_and_agent_tools_are_declared_as_published(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    out, err = capsys.readouterr()
    tools = {t["name"]: t for t in json.loads(out)["tools"]}
    assert tools == {
        "lookup": {
            "name": "lookup",
            "description": "Look up an order.",
            # options[name] wins over the tool's own metadata.
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
            "outputSchemaJson": json.dumps(
                {"additionalProperties": {"type": "string"}, "type": "object"}, sort_keys=True
            ),
            "annotations": {"readOnly": True},
            "display": "TOOL_DISPLAY_SUMMARY",
            "origin": "pydantic_agents.py#TOOLS.tools.lookup",
        },
        "ping": {
            "name": "ping",
            "description": "Check the service.",
            "inputSchemaJson": json.dumps(
                {"additionalProperties": False, "properties": {}, "type": "object"},
                sort_keys=True,
            ),
            "outputSchemaJson": '{"type": "string"}',
            "display": "TOOL_DISPLAY_FULL",
            "origin": "pydantic_agents.py#responder.tools.ping",
        },
    }
    assert "tool   ping pydantic_agents.py#responder.tools.ping" in err
