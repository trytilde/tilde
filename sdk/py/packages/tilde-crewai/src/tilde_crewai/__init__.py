"""Tilde adapter for CrewAI: typed context to CrewAI messages, channel tools to CrewAI tools."""

from tilde_crewai.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    AttachmentPart,
    AttachmentResult,
    convert_attachment,
    normalize_media_type,
)
from tilde_crewai.messages import MessageHandlers, convert_to_crewai_messages
from tilde_crewai.tools import convert_to_crewai_tools

__all__ = [
    "AttachmentConversion",
    "AttachmentHandler",
    "AttachmentPart",
    "AttachmentResult",
    "MessageHandlers",
    "convert_attachment",
    "convert_to_crewai_messages",
    "convert_to_crewai_tools",
    "normalize_media_type",
]
