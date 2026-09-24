#!/usr/bin/env python3
"""Opt-in development agents (one per SDK adapter). SOPS material stays in the child environment."""
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys

enabled = os.environ.get("REGISTER_DEV_AGENTS", "0")
if enabled == "0":
    sys.exit(0)
if enabled != "1":
    sys.exit("REGISTER_DEV_AGENTS must be 0 or 1")
key = os.environ.get("OPENAI_API_KEY")
if not key:
    result = subprocess.run(
        ["sops", "decrypt", "--extract", '["openai_api_key"]', "--output-type", "json",
         str(Path(__file__).resolve().parent.parent / "secrets.enc.yaml")],
        capture_output=True, text=True,
    )
    if result.returncode:
        sys.exit("Cannot load openai_api_key from SOPS. Refresh AWS access or run task secrets:load with a valid session.")
    try:
        key = json.loads(result.stdout)
    except json.JSONDecodeError:
        key = result.stdout.strip()
if not isinstance(key, str) or not key.strip():
    sys.exit("SOPS openai_api_key is missing or empty")
root = Path(__file__).resolve().parent.parent
selected = [key.strip() for key in os.environ.get("DEV_AGENTS", "").split(",") if key.strip()]
python_agents = not selected or any(key.startswith("py-") for key in selected)
if python_agents and not shutil.which("uv"):
    sys.exit("uv is required for the Python example agents; install it or set DEV_AGENTS to TypeScript examples")
if sys.argv[1:] == ["--check"]:
    sys.exit(0)
if python_agents:
    # The Python examples run from the uv workspace with contracts generated from proto/.
    subprocess.run([str(root / "sdk/py/scripts/sync.sh")], check=True)
    subprocess.run(["uv", "run", "--no-sync", "python", "scripts/generate.py"], cwd=root / "sdk/py", check=True)
os.execvpe("cargo", ["cargo", "run", "--bin", "tilde", "--", "dev-agent"], {**os.environ, "OPENAI_API_KEY": key})
