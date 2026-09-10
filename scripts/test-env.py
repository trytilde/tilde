#!/usr/bin/env python3
"""Exercise secret conversion and the real Task dotenv parser/precedence without cloud access."""
import importlib.util
import json
import io
from unittest.mock import patch
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("load_secrets", ROOT / "scripts/load-secrets.py")
loader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loader)
with tempfile.TemporaryDirectory(prefix="tilde-env-") as directory:
    root = Path(directory)
    fixture = 'first line\n"quoted" \\backslash $HOME ${UNSET} $(false) `false`\nlast line'
    loader.write_env(root / ".env.secrets", {"FIXTURE": fixture, "PRECEDENCE": "dev-secret"})
    loader.write_env(root / ".env.secrets.test", {"FIXTURE": fixture, "PRECEDENCE": "test-secret"})
    assert (root / ".env.secrets").stat().st_mode & 0o777 == 0o600
    (root / ".env").write_text('PRECEDENCE=dev-default\nMODE=dev\n')
    (root / ".env.test").write_text('PRECEDENCE=test-default\nMODE=test\n')
    # Reuse the actual project's dotenv declarations; only the leaf command is a fixture.
    taskfile = (ROOT / "Taskfile.yml").read_text()
    global_dotenv = next(line for line in taskfile.splitlines() if line.startswith("dotenv:"))
    test_dotenv = next(line.strip() for line in taskfile.split("  test:chat:\n", 1)[1].splitlines() if "dotenv:" in line)
    (root / "Taskfile.yml").write_text('version: "3"\n' + global_dotenv + '\ntasks:\n  dev:\n    cmds: ["python3 capture.py"]\n  test:\n    '+test_dotenv+'\n    cmds: ["python3 capture.py"]\n')
    (root / "capture.py").write_text('import os,json\nfrom pathlib import Path\nPath("result.json").write_text(json.dumps({k:os.environ[k] for k in ["FIXTURE","PRECEDENCE","MODE"]}))\n')
    env = {k:v for k,v in os.environ.items() if k not in {"FIXTURE","PRECEDENCE","MODE"}}
    for mode in ["dev", "test"]:
        subprocess.run(["task", "--dir", str(root), mode], env=env, check=True, capture_output=True)
        result = json.loads((root / "result.json").read_text())
        assert result == {"FIXTURE":fixture,"PRECEDENCE":f"{mode}-secret","MODE":mode}, 'dotenv round-trip or dev/test precedence failed'
        subprocess.run(["task", "--dir", str(root), mode], env={**env,"PRECEDENCE":"exported"}, check=True, capture_output=True)
        assert json.loads((root / "result.json").read_text())["PRECEDENCE"] == "exported"
    (root / ".env.local").write_text('PRECEDENCE=private-dev\n')
    (root / ".env.test.local").write_text('PRECEDENCE=private-test\n')
    for mode in ["dev", "test"]:
        subprocess.run(["task", "--dir", str(root), mode], env=env, check=True, capture_output=True)
        assert json.loads((root / "result.json").read_text())["PRECEDENCE"] == f"private-{mode}"
    loader.ROOT = root
    document = {"database_url":"must-not-migrate", "unrelated_secret":"must-not-migrate",
                "linq_api_token":"dev-token", "test":{"linq_api_token":"test-token",
                "e2e_mcp_whatsapp_credential_json":json.dumps({"access_token":"meta-token"})}}
    with patch("sys.argv", ["load-secrets.py", "--stdin-json"]), patch("sys.stdin", io.StringIO(json.dumps(document))):
        loader.main()
    dev = (root / ".env.secrets").read_text()
    test = (root / ".env.secrets.test").read_text()
    assert "must-not-migrate" not in dev + test
    assert 'LINQ_API_TOKEN="dev-token"' in dev
    assert 'LINQ_API_TOKEN="test-token"' in test
    assert 'CHAT_TEST_WHATSAPP_ACCESS_TOKEN="meta-token"' in test
    document["test"]["e2e_mcp_whatsapp_credential_json"] = '{"bad-key":"invalid"}'
    with patch("sys.argv", ["load-secrets.py", "--stdin-json"]), patch("sys.stdin", io.StringIO(json.dumps(document))):
        try:
            loader.main()
            raise AssertionError("Malformed secret mapping was accepted")
        except ValueError:
            pass
    assert (root / ".env.secrets").read_text() == dev
    assert (root / ".env.secrets.test").read_text() == test
print("Secret dotenv round-trip, file permissions, environment isolation and overrides passed")
