#!/usr/bin/env python3
"""Run the isolated trace demo with only the OpenAI key extracted from SOPS.

The key stays in memory and the child environment, never a dotenv file or command
argument. LANGSMITH_API_KEY is supplied separately by the user.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def main():
    result = subprocess.run(
        ["sops", "decrypt", "--extract", '["openai_api_key"]', "--output-type", "json",
         str(ROOT / "secrets.enc.yaml")],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, text=True,
    )
    if result.returncode and "ExpiredToken" in result.stderr:
        sys.exit("SOPS AWS credentials have expired. Refresh the workspace AWS session and retry.")
    if result.returncode:
        sys.exit("Could not load the OpenAI key from SOPS. Check local SOPS/AWS access.")
    try:
        key = json.loads(result.stdout)
    except json.JSONDecodeError:
        key = result.stdout.strip()
    if not isinstance(key, str) or not key.strip():
        sys.exit("SOPS openai_api_key is missing or empty.")
    if sys.argv[1:] == ["--check"]:
        print("OpenAI key is available from SOPS (value not displayed).")
        print("LangSmith key is " + ("available." if os.environ.get("LANGSMITH_API_KEY") else "still required."))
        return
    env = {**os.environ, "OPENAI_API_KEY": key, "LANGSMITH_TRACING": "true"}
    env.setdefault("LANGSMITH_ENDPOINT", "https://api.smith.langchain.com")
    env.setdefault("LANGSMITH_PROJECT", "test")
    if not env.get("LANGSMITH_API_KEY"):
        sys.exit("Set LANGSMITH_API_KEY before running the demo.")
    sys.exit(subprocess.call(
        ["node", str(ROOT / "sdk/ts/langsmith-agent/dist/index.js"), *sys.argv[1:]],
        env=env, cwd=ROOT,
    ))


if __name__ == "__main__":
    main()
