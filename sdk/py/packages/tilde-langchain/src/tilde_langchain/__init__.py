"""Tilde adapter for LangChain / LangGraph.

Core history and delivery live in ``tilde``; this package converts typed context to LangChain
messages, exposes channel tools as LangChain ``BaseTool`` instances, brings the invocation to a
module-level ``create_agent`` graph through ``tilde_middleware()`` and lets ``tilde deploy``
discover its prompts.
"""

from tilde_langchain.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    ContentBlock,
    convert_attachment,
)
from tilde_langchain.discover import discover
from tilde_langchain.messages import MessageHandlers, convert_to_langchain_messages
from tilde_langchain.middleware import TildeMiddleware, tilde_middleware
from tilde_langchain.tools import ChannelToolAdapter, convert_to_langchain_tools, with_tilde_tools

__all__ = [
    "AttachmentConversion",
    "AttachmentHandler",
    "ChannelToolAdapter",
    "ContentBlock",
    "MessageHandlers",
    "TildeMiddleware",
    "convert_attachment",
    "convert_to_langchain_messages",
    "convert_to_langchain_tools",
    "discover",
    "tilde_middleware",
    "with_tilde_tools",
]
