"""`tilde deploy --dry-run` on a LangChain project, through the installed `tilde.discover`
entry point: create_agent's system prompt, dynamic prompts and prompt templates."""

from __future__ import annotations

import json
import os
import sys

import pytest

from tilde import deploy

AGENT = """
from langchain.agents import create_agent
from langchain.agents.middleware import dynamic_prompt
from langchain_core.language_models.fake_chat_models import GenericFakeChatModel


@dynamic_prompt
def personalized(request):
    return f"Help {request.runtime.context['user']}."


model = GenericFakeChatModel(messages=iter([]))
support = create_agent(model, system_prompt="You answer support questions.",
                       middleware=[personalized])
"""
MAIN = """
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder, PromptTemplate

from lc_support.agent import support  # noqa: F401

SUMMARY = PromptTemplate.from_template("Summarize {text}")
CHAT = ChatPromptTemplate.from_messages(
    [("system", "Hi {{name}}"), MessagesPlaceholder("history")], template_format="mustache"
)
# What `langsmith.Client().pull_prompt` returns: versioned by the hub.
HUB = PromptTemplate.from_template("Classify {text}", metadata={"lc_hub_repo": "classify"})
"""


@pytest.fixture
def project(tmp_path, monkeypatch):
    (tmp_path / "lc_support").mkdir()
    (tmp_path / "lc_support" / "__init__.py").write_text("")
    (tmp_path / "lc_support" / "agent.py").write_text(AGENT)
    (tmp_path / "main.py").write_text(MAIN)
    monkeypatch.chdir(tmp_path)
    monkeypatch.syspath_prepend(str(tmp_path))
    before = set(sys.modules)
    yield tmp_path
    for name in set(sys.modules) - before:
        del sys.modules[name]
    os.environ.pop("TILDE_DISCOVERY", None)


def test_agent_prompts_and_templates_are_declared(project, capsys):
    assert deploy.main(["main.py", "--dry-run"]) == 0
    out, err = capsys.readouterr()
    prompts = {p["name"]: p for p in json.loads(out)["prompts"]}
    assert {name: (p["format"], p["origin"]) for name, p in prompts.items()} == {
        "support/system_prompt": ("PROMPT_FORMAT_PLAIN", "main.py#support.system_prompt"),
        # Found in the graph, and again as a module global: one declaration.
        "personalized/system_prompt": (
            "PROMPT_FORMAT_DYNAMIC",
            "main.py#support.middleware.personalized",
        ),
        "SUMMARY": ("PROMPT_FORMAT_BRACES", "main.py#SUMMARY"),
        "CHAT/0.system": ("PROMPT_FORMAT_MUSTACHE", "main.py#CHAT.messages[0]"),
    }
    assert prompts["support/system_prompt"]["template"] == "You answer support questions."
    assert prompts["personalized/system_prompt"]["template"].startswith("@dynamic_prompt\ndef ")
    assert "main.py#HUB: LangSmith hub prompt is versioned by the hub" in err


TOOLS_AGENT = """
from langchain.agents import create_agent
from langchain_core.language_models.fake_chat_models import GenericFakeChatModel
from langchain_core.tools import tool

from tilde import BundledOptions, define_tools


@tool
def lookup(order_id: str) -> str:
    \"\"\"Look up an order.\"\"\"
    return "shipped"


@tool
def ping() -> str:
    \"\"\"Check the service.\"\"\"
    return "pong"


lookup.metadata = {"tilde": BundledOptions(summary="Looked up", display="summary")}
TOOLS = define_tools([lookup])
model = GenericFakeChatModel(messages=iter([]))
support = create_agent(model, tools=[ping])
"""


def test_bundled_and_agent_tools_are_declared_as_published(tmp_path, monkeypatch, capsys):
    (tmp_path / "tools_main.py").write_text(TOOLS_AGENT)
    monkeypatch.chdir(tmp_path)
    monkeypatch.syspath_prepend(str(tmp_path))
    assert deploy.main(["tools_main.py", "--dry-run"]) == 0
    tools = {t["name"]: t for t in json.loads(capsys.readouterr().out)["tools"]}
    assert set(tools) == {"lookup", "ping"}
    assert tools["lookup"]["summary"] == "Looked up"
    assert tools["lookup"]["display"] == "TOOL_DISPLAY_SUMMARY"
    assert tools["lookup"]["origin"] == "tools_main.py#TOOLS.tools.lookup"
    assert json.loads(tools["lookup"]["inputSchemaJson"])["properties"] == {
        "order_id": {"type": "string"}
    }
    assert tools["ping"]["description"] == "Check the service."
    assert tools["ping"]["origin"].startswith("tools_main.py#support.")
