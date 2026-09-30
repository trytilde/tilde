#!/usr/bin/env bash
# Sync the Python SDK workspace from uv.lock. CrewAI pins openai<3 while the OpenAI Agents SDK
# needs openai>=3 (declared under [tool.uv] conflicts), so CrewAI gets its own environment.
set -euo pipefail
cd "$(dirname "$0")/.."
shared=(--package tilde-py-workspace --package trytilde)
uv sync --frozen "${shared[@]}" \
  --package trytilde-langchain --package trytilde-pydantic-ai \
  --package trytilde-openai-agents --package trytilde-agno \
  --package example-agent-langchain --package example-agent-pydantic-ai \
  --package example-agent-openai-agents --package example-agent-agno \
  --package example-tool-server
UV_PROJECT_ENVIRONMENT=.venv-crewai uv sync --frozen "${shared[@]}" \
  --package trytilde-crewai --package example-agent-crewai
