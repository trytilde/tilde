#!/usr/bin/env python3
"""Opt-in development agents (one per SDK adapter). OPENAI_API_KEY comes from .env.local."""
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
key = os.environ.get("OPENAI_API_KEY", "")
if not key.strip():
    sys.exit("Set OPENAI_API_KEY in .env.local to run the example agents")
# The examples live in open-source Tilde's sdk/; Tilde Cloud points TILDE_SDK_ROOT at a checkout of it.
root = Path(os.environ.get("TILDE_SDK_ROOT") or Path(__file__).resolve().parent.parent).resolve()
if not (root / "sdk").is_dir():
    sys.exit(f"{root}/sdk not found; set TILDE_SDK_ROOT to a checkout of trytilde/tilde")
selected = [key.strip() for key in os.environ.get("DEV_AGENTS", "").split(",") if key.strip()]
python_agents = not selected or any(key.startswith("py-") for key in selected)
if python_agents and not shutil.which("uv"):
    sys.exit("uv is required for the Python example agents; install it or set DEV_AGENTS to TypeScript examples")
if sys.argv[1:] == ["--check"]:
    sys.exit(0)
subprocess.run(["pnpm", "--dir", str(root / "sdk/ts"), "install", "--frozen-lockfile"], check=True)
subprocess.run(["pnpm", "--dir", str(root / "sdk/ts"), "build"], check=True)
if python_agents:
    # The Python examples run from the uv workspace with contracts generated from proto/.
    subprocess.run([str(root / "sdk/py/scripts/sync.sh")], check=True)
    subprocess.run(["uv", "run", "--no-sync", "python", "scripts/generate.py"], cwd=root / "sdk/py", check=True)
os.execvpe("cargo", ["cargo", "run", "--bin", "tilde", "--", "dev-agent"], {**os.environ, "OPENAI_API_KEY": key, "TILDE_SDK_ROOT": str(root)})
