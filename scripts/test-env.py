#!/usr/bin/env python3
"""Exercise the real Task dotenv declarations: private overrides, dev/test isolation, exports win."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory(prefix="tilde-env-") as directory:
    root = Path(directory)
    fixture = 'first line\n"quoted" \\backslash $HOME ${UNSET} $(false) `false`\nlast line'
    escaped = fixture.replace("\\", "\\\\").replace('"', '\\"').replace("$", "\\$")
    (root / ".env").write_text("PRECEDENCE=dev-default\nMODE=dev\n")
    (root / ".env.test").write_text("PRECEDENCE=test-default\nMODE=test\n")
    (root / ".env.local").write_text(f'PRECEDENCE=private-dev\nFIXTURE="{escaped}"\n')
    (root / ".env.test.local").write_text(f'PRECEDENCE=private-test\nFIXTURE="{escaped}"\n')
    # Reuse the actual project's dotenv declarations; only the leaf command is a fixture.
    taskfile = (ROOT / "Taskfile.yml").read_text()
    global_dotenv = next(line for line in taskfile.splitlines() if line.startswith("dotenv:"))
    test_dotenv = next(line.strip() for line in taskfile.split("  test:chat:\n", 1)[1].splitlines() if "dotenv:" in line)
    (root / "Taskfile.yml").write_text('version: "3"\n' + global_dotenv + '\ntasks:\n  dev:\n    cmds: ["python3 capture.py"]\n  test:\n    '+test_dotenv+'\n    cmds: ["python3 capture.py"]\n')
    (root / "capture.py").write_text('import os,json\nfrom pathlib import Path\nPath("result.json").write_text(json.dumps({k:os.environ.get(k) for k in ["FIXTURE","PRECEDENCE","MODE"]}))\n')
    env = {k: v for k, v in os.environ.items() if k not in {"FIXTURE", "PRECEDENCE", "MODE"}}
    for mode in ["dev", "test"]:
        subprocess.run(["task", "--dir", str(root), mode], env=env, check=True, capture_output=True)
        result = json.loads((root / "result.json").read_text())
        assert result == {"FIXTURE": fixture, "PRECEDENCE": f"private-{mode}", "MODE": mode}, "dotenv round-trip or dev/test precedence failed"
        subprocess.run(["task", "--dir", str(root), mode], env={**env, "PRECEDENCE": "exported"}, check=True, capture_output=True)
        assert json.loads((root / "result.json").read_text())["PRECEDENCE"] == "exported"
print("Dotenv round-trip, private overrides, dev/test isolation and exported overrides passed")
