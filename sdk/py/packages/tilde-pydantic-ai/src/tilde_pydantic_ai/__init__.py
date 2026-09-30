"""Tilde adapter for Pydantic AI: typed context to model messages, channel tools to tools, the
per-run ``tilde_pydantic_ai`` capability and ``tilde deploy`` discovery of module-level agents."""

from tilde_pydantic_ai.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    convert_attachment,
)
from tilde_pydantic_ai.discover import discover
from tilde_pydantic_ai.invocation import TildeInvocation, tilde_pydantic_ai
from tilde_pydantic_ai.messages import (
    MESSAGE_ID_KEY,
    MessageHandlers,
    convert_to_pydantic_ai_messages,
    message_id,
    text_message,
)
from tilde_pydantic_ai.tools import convert_to_pydantic_ai_tools, with_tilde_tools

__all__ = [
    "MESSAGE_ID_KEY",
    "AttachmentConversion",
    "AttachmentHandler",
    "MessageHandlers",
    "TildeInvocation",
    "convert_attachment",
    "discover",
    "convert_to_pydantic_ai_messages",
    "convert_to_pydantic_ai_tools",
    "message_id",
    "text_message",
    "tilde_pydantic_ai",
    "with_tilde_tools",
]
