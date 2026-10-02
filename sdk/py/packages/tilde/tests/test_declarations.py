"""`tilde-declarations` against a small project: entry loading, module scanning,
core define_* recognition, entry point discoverers and the proto JSON it prints."""

from __future__ import annotations

import base64
import importlib.metadata
import json
import os
import sys
import textwrap

import pytest

from tilde import declarations as deploy

ENTRY = """
import tilde
from helpers import GREETING  # a project module: scanned too

SKILLS = tilde.define_skills("skills")
NOTES = tilde.define_skill("notes", "Keeps notes.", "Write notes down.", {"a.txt": "x"})


class Framework:
    instructions = "Be kind."


support = Framework()
# Dials nothing under discovery.
tilde.run_connected_agent(run=None)

if __name__ == "__main__":
    raise SystemExit("the __main__ block must not run")
"""
HELPERS = """
import tilde

GREETING = tilde.define_prompt(
    "greeting",
    template="Hello {{name}}\\n{{> rules}}",
    sections={"rules": "Be brief."},
    config={"model": "claude-sonnet-5", "temperature": 0.2},
)
"""
DISCOVERER = """
from tilde._tools import ToolSpec
from tilde.discovery import PROMPT_FORMAT_PLAIN, Discovered, declared_prompt, declared_tool


def discover(value, context):
    if type(value).__name__ == "Tool":
        spec = ToolSpec(value.name, value.description, {"type": "object"})
        return Discovered(tools=[declared_tool(spec, f"{context.origin()}")])
    if type(value).__name__ != "Framework":
        return None
    context.warn("framework tools are not declared")
    origin = f"{context.relative(context.root / 'main.py')}#{context.name}.instructions"
    return Discovered(
        prompts=[
            declared_prompt(f"{context.name}/instructions", value.instructions,
                            PROMPT_FORMAT_PLAIN, origin)
        ]
    )
"""


@pytest.fixture
def project(tmp_path, monkeypatch):
    (tmp_path / "main.py").write_text(ENTRY)
    (tmp_path / "helpers.py").write_text(HELPERS)
    plugins = tmp_path / "plugins"
    plugins.mkdir()
    (plugins / "fake_discoverer.py").write_text(DISCOVERER)
    skill = tmp_path / "skills" / "triage"
    (skill / "__pycache__").mkdir(parents=True)
    (skill / "SKILL.md").write_text(
        textwrap.dedent("""\
        ---
        name: triage
        description: Sort incoming requests.
        ---
        Read the request, then run run.sh.
        """)
    )
    (skill / "run.sh").write_text("#!/bin/sh\necho ok\n")
    os.chmod(skill / "run.sh", 0o755)
    (skill / "logo.png").write_bytes(b"\x89PNG\r\n\x1a\n\xff\x00")
    (skill / "__pycache__" / "x.pyc").write_bytes(b"\x00")
    (tmp_path / "skills" / "not-a-skill").mkdir()

    point = importlib.metadata.EntryPoint(
        name="fake", value="fake_discoverer:discover", group="tilde.discover"
    )
    monkeypatch.setattr(
        importlib.metadata,
        "entry_points",
        lambda group: [point] if group == "tilde.discover" else [],
    )
    monkeypatch.syspath_prepend(str(plugins))
    monkeypatch.chdir(tmp_path)
    monkeypatch.delenv("TILDE_DISCOVERY", raising=False)
    before = set(sys.modules)
    yield tmp_path
    for name in set(sys.modules) - before:
        del sys.modules[name]
    os.environ.pop("TILDE_DISCOVERY", None)


def test_prints_every_declaration_as_proto_json(project, capsys):
    assert deploy.main(["main.py"]) == 0
    out, err = capsys.readouterr()
    declarations = json.loads(out)

    prompts = {p["name"]: p for p in declarations["prompts"]}
    assert prompts["greeting"] == {
        "name": "greeting",
        "format": "PROMPT_FORMAT_MUSTACHE",
        "template": "Hello {{name}}\n{{> rules}}",
        "sections": [{"name": "rules", "content": "Be brief."}],
        "config": '{"model":"claude-sonnet-5","temperature":0.2}',
        # The hash shared with the engine and the TypeScript SDK.
        "hash": "985e00ff1447c634a4fbe5dca489e677fbdf175148e3a9933e05c884ebf9e691",
        "origin": "helpers.py#greeting",
    }
    assert prompts["support/instructions"]["format"] == "PROMPT_FORMAT_PLAIN"
    assert prompts["support/instructions"]["origin"] == "main.py#support.instructions"
    assert len(prompts) == 2

    skills = {s["name"]: s for s in declarations["skills"]}
    assert set(skills) == {"triage", "notes"}
    triage = skills["triage"]
    assert triage["origin"] == "skills/triage"
    files = {f["path"]: f for f in triage["files"]}
    assert set(files) == {"SKILL.md", "logo.png", "run.sh"}  # __pycache__ skipped
    assert files["run.sh"] == {
        "path": "run.sh",
        "executable": True,
        "content": "#!/bin/sh\necho ok\n",
    }
    assert base64.b64decode(files["logo.png"]["data"]) == b"\x89PNG\r\n\x1a\n\xff\x00"
    assert "executable" not in files["SKILL.md"]
    notes = skills["notes"]
    assert notes["origin"] == "main.py#notes"
    assert notes["files"][0]["content"].startswith("---\nname: notes\ndescription: Keeps notes.")

    assert "prompt greeting (mustache) helpers.py#greeting" in err
    assert "warning: framework tools are not declared" in err


def test_conflicting_prompts_fail_the_read(project, capsys):
    (project / "other.py").write_text(
        "import tilde\nGREETING = tilde.define_prompt('greeting', template='Hi')\n"
    )
    with open(project / "main.py", "a") as entry:
        entry.write("import other\n")
    assert deploy.main(["main.py"]) == 1
    assert "Prompt greeting is declared twice with different content" in capsys.readouterr().err


TOOL_CLASS = (
    "class Tool:\n"
    "    def __init__(self, name, description):\n"
    "        self.name, self.description = name, description\n"
)


def test_tools_nobody_recognises_warn_and_conflicting_tools_fail(project, capsys):
    # A define_tools of objects no installed adapter knows is skipped with a warning.
    (project / "stray.py").write_text("import tilde\nTOOLS = tilde.define_tools([print])\n")
    with open(project / "main.py", "a") as entry:
        entry.write("import stray\n")
    assert deploy.main(["main.py"]) == 0
    out, err = capsys.readouterr()
    assert json.loads(out).get("tools", []) == []
    assert "no installed adapter recognises these tools" in err

    # The same tool name declared with two different definitions fails the deploy.
    (project / "tools_a.py").write_text(TOOL_CLASS + "LOOKUP = Tool('lookup', 'Find it.')\n")
    (project / "tools_b.py").write_text(TOOL_CLASS + "LOOKUP = Tool('lookup', 'Look it up.')\n")
    with open(project / "main.py", "a") as entry:
        entry.write("import tools_a\nimport tools_b\n")
    assert deploy.main(["main.py"]) == 1
    assert "Tool lookup is declared twice with different definitions" in capsys.readouterr().err
