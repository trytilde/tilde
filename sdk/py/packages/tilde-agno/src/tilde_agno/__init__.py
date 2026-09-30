"""Tilde adapter for Agno: typed context to Agno messages, channel tools to Agno functions, the
per-run ``tilde_agno`` options, registry skills through ``TildeSkills`` and ``tilde deploy``
discovery of module-level agents and teams."""

from tilde_agno.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    AttachmentPart,
    AttachmentResult,
    convert_attachment,
    normalize_media_type,
)
from tilde_agno.discover import discover
from tilde_agno.invocation import TildeSkills, tilde_agno
from tilde_agno.messages import MessageHandlers, convert_to_agno_messages
from tilde_agno.tools import convert_to_agno_tools, with_tilde_tools

__all__ = [
    "AttachmentConversion",
    "AttachmentHandler",
    "AttachmentPart",
    "AttachmentResult",
    "MessageHandlers",
    "TildeSkills",
    "convert_attachment",
    "convert_to_agno_messages",
    "convert_to_agno_tools",
    "discover",
    "normalize_media_type",
    "tilde_agno",
    "with_tilde_tools",
]
