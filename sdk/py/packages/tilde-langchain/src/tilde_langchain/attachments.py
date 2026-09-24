"""Attachment hydration into LangChain standard content blocks (langchain-core 1.x)."""

from __future__ import annotations

import base64
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

from tilde import ConversationMessage, DownloadedAttachment
from tilde.types.v1.chat_pb2 import Attachment

ContentBlock = dict[str, Any]
"""A ``langchain_core.messages.content`` block: ``{"type": "text", ...}``, ``image``, ``file``."""

_TEXT_TYPES = {"application/json", "application/xml"}
_FILE_TYPES = {"image/jpeg", "image/png", "image/gif", "image/webp", "application/pdf"}
_EXTENSIONS = {
    "jpg": "image/jpeg",
    "jpeg": "image/jpeg",
    "png": "image/png",
    "gif": "image/gif",
    "webp": "image/webp",
    "pdf": "application/pdf",
    "txt": "text/plain",
    "md": "text/plain",
    "csv": "text/plain",
    "log": "text/plain",
    "json": "application/json",
    "xml": "application/xml",
}


@dataclass(slots=True)
class AttachmentConversion:
    message: ConversationMessage
    attachment: Attachment
    download: Callable[[], Awaitable[DownloadedAttachment]]
    """Uses the current invocation's authenticated, thread-scoped attachment API."""


AttachmentResult = ContentBlock | list[ContentBlock] | None
AttachmentHandler = Callable[[AttachmentConversion], AttachmentResult | Awaitable[AttachmentResult]]


async def convert_attachment(input: AttachmentConversion) -> AttachmentResult:
    """Hydrate images/PDFs as base64 blocks and textual files as actual text, not filenames."""
    attachment = input.attachment
    media_type = normalize_media_type(attachment.media_type, attachment.filename)
    text = media_type.startswith("text/") or media_type in _TEXT_TYPES
    file = media_type in _FILE_TYPES
    if not text and not file:
        return {
            "type": "text",
            "text": (
                f"Attached file: {attachment.filename} ({media_type}). "
                "A custom attachment handler is needed to read this format."
            ),
        }
    result = await input.download()
    if result.attachment.id != attachment.id:
        raise RuntimeError("Attachment download returned a different file")
    if text:
        content = result.content.decode("utf-8", errors="replace")
        return {"type": "text", "text": f"Attached file: {attachment.filename}\n{content}"}
    encoded = base64.b64encode(result.content).decode("ascii")
    if media_type.startswith("image/"):
        return {"type": "image", "mime_type": media_type, "base64": encoded}
    # ``filename`` is not part of FileContentBlock but the OpenAI translator forwards it.
    return {
        "type": "file",
        "mime_type": media_type,
        "base64": encoded,
        "filename": attachment.filename,
    }


def normalize_media_type(value: str, filename: str) -> str:
    media_type = value.split(";")[0].strip().lower()
    if media_type and media_type != "application/octet-stream":
        return media_type
    extension = filename.rsplit(".", 1)[-1].lower() if "." in filename else ""
    return _EXTENSIONS.get(extension, media_type or "application/octet-stream")
