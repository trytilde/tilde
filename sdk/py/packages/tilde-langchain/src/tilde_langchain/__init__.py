"""Tilde adapter for LangChain / LangGraph.

Core history and delivery live in ``tilde``; this package converts typed context to LangChain
messages and exposes channel tools as LangChain ``BaseTool`` instances.
"""

from tilde_langchain.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    ContentBlock,
    convert_attachment,
)
from tilde_langchain.messages import MessageHandlers, convert_to_langchain_messages
from tilde_langchain.tools import ChannelToolAdapter, convert_to_langchain_tools

__all__ = [
    "AttachmentConversion",
    "AttachmentHandler",
    "ChannelToolAdapter",
    "ContentBlock",
    "MessageHandlers",
    "convert_attachment",
    "convert_to_langchain_messages",
    "convert_to_langchain_tools",
]
