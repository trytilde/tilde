#!/usr/bin/env python3
"""Run the isolated trace demo with OPENAI_API_KEY and LANGSMITH_API_KEY from the environment.

Task loads them from .env.local or .env.langsmith; neither value is printed.
"""
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def main():
    if sys.argv[1:] == ["--check"]:
        for name in ["OPENAI_API_KEY", "LANGSMITH_API_KEY"]:
            print(f"{name} is " + ("available." if os.environ.get(name) else "still required."))
        return
    if not os.environ.get("OPENAI_API_KEY"):
        sys.exit("Set OPENAI_API_KEY before running the demo.")
    env = {**os.environ, "LANGSMITH_TRACING": "true"}
    env.setdefault("LANGSMITH_ENDPOINT", "https://api.smith.langchain.com")
    env.setdefault("LANGSMITH_PROJECT", "test")
    if not env.get("LANGSMITH_API_KEY"):
        sys.exit("Set LANGSMITH_API_KEY before running the demo.")
    sys.exit(subprocess.call(
        ["node", str(ROOT / "sdk/ts/examples/langsmith-agent/dist/index.js"), *sys.argv[1:]],
        env=env, cwd=ROOT,
    ))


if __name__ == "__main__":
    main()
