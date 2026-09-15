#!/usr/bin/env python3
"""Opt-in development agent. SOPS material stays in the child environment, never arguments."""
import json
import os
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
if sys.argv[1:] == ["--check"]:
    sys.exit(0)
os.execvpe("cargo", ["cargo", "run", "--bin", "tilde", "--", "dev-agent"], {**os.environ, "OPENAI_API_KEY": key})
