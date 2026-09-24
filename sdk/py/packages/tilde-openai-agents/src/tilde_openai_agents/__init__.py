"""Tilde adapter for the OpenAI Agents SDK (``openai-agents``).

Core history and delivery live in ``trytilde``. This package converts typed context into
Responses API input items and exposes channel tools as ``agents.FunctionTool`` instances.
"""

from tilde_openai_agents.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    convert_attachment,
)
from tilde_openai_agents.messages import MessageHandlers, convert_to_openai_agents_messages
from tilde_openai_agents.tools import convert_to_openai_agents_tools

__all__ = [
    "AttachmentConversion",
    "AttachmentHandler",
    "MessageHandlers",
    "convert_attachment",
    "convert_to_openai_agents_messages",
    "convert_to_openai_agents_tools",
]
