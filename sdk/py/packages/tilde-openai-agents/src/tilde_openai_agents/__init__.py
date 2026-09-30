"""Tilde adapter for the OpenAI Agents SDK (``openai-agents``).

Core history and delivery live in ``trytilde``. This package converts typed context into
Responses API input items, exposes channel tools as ``agents.FunctionTool`` instances, bundles
the per-invocation run options (``tilde_openai_agents``) and lets ``tilde deploy`` discover
module-level agents' prompts and skills.
"""

from tilde_openai_agents.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    convert_attachment,
)
from tilde_openai_agents.discover import discover
from tilde_openai_agents.invocation import tilde_openai_agents
from tilde_openai_agents.messages import MessageHandlers, convert_to_openai_agents_messages
from tilde_openai_agents.tools import convert_to_openai_agents_tools, with_tilde_tools

__all__ = [
    "AttachmentConversion",
    "AttachmentHandler",
    "MessageHandlers",
    "convert_attachment",
    "convert_to_openai_agents_messages",
    "convert_to_openai_agents_tools",
    "discover",
    "tilde_openai_agents",
    "with_tilde_tools",
]
