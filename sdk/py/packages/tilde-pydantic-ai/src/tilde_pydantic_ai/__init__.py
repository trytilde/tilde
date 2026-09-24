"""Tilde adapter for Pydantic AI: typed context to model messages, channel tools to tools."""

from tilde_pydantic_ai.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    convert_attachment,
)
from tilde_pydantic_ai.messages import (
    MESSAGE_ID_KEY,
    MessageHandlers,
    convert_to_pydantic_ai_messages,
    message_id,
    text_message,
)
from tilde_pydantic_ai.tools import convert_to_pydantic_ai_tools

__all__ = [
    "MESSAGE_ID_KEY",
    "AttachmentConversion",
    "AttachmentHandler",
    "MessageHandlers",
    "convert_attachment",
    "convert_to_pydantic_ai_messages",
    "convert_to_pydantic_ai_tools",
    "message_id",
    "text_message",
]
