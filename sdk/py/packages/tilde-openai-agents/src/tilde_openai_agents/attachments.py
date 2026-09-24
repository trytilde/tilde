"""Attachment hydration into Responses API content parts."""

from __future__ import annotations

import base64
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

from tilde import ConversationMessage, DownloadedAttachment
from tilde.types.v1.chat_pb2 import Attachment

# Responses API content parts: input_text, input_image, input_file (openai TypedDicts).
ContentPart = dict[str, Any]
Conversion = ContentPart | list[ContentPart] | None
AttachmentHandler = Callable[["AttachmentConversion"], Conversion | Awaitable[Conversion]]

_TEXT_TYPES = {"application/json", "application/xml"}
_IMAGE_TYPES = {"image/jpeg", "image/png", "image/gif", "image/webp"}
_FILE_TYPES = {"application/pdf"}


@dataclass(slots=True)
class AttachmentConversion:
    message: ConversationMessage
    attachment: Attachment
    download: Callable[[], Awaitable[DownloadedAttachment]]
    """Uses the current invocation's authenticated, thread-scoped attachment API."""


async def convert_attachment(input: AttachmentConversion) -> ContentPart:
    """Hydrate images as ``input_image``, PDFs as ``input_file`` and textual files as real text.

    Assistant messages cannot carry image or file parts in the Responses API, so the agent's
    own attachments are described instead of downloaded.
    """
    attachment = input.attachment
    media_type = normalize_media_type(attachment.media_type, attachment.filename)
    text = media_type.startswith("text/") or media_type in _TEXT_TYPES
    image = media_type in _IMAGE_TYPES
    file = media_type in _FILE_TYPES
    if input.message.role == "assistant" and not text:
        return _text(f"Attached file: {attachment.filename} ({media_type}).")
    if not (text or image or file):
        return _text(
            f"Attached file: {attachment.filename} ({media_type}). "
            "A custom attachment handler is needed to read this format."
        )
    result = await input.download()
    if result.attachment.id != attachment.id:
        raise RuntimeError("Attachment download returned a different file")
    if text:
        content = result.content.decode("utf-8", errors="replace")
        return _text(f"Attached file: {attachment.filename}\n{content}")
    data_url = f"data:{media_type};base64,{base64.b64encode(result.content).decode('ascii')}"
    if image:
        return {"type": "input_image", "detail": "auto", "image_url": data_url}
    return {"type": "input_file", "filename": attachment.filename, "file_data": data_url}


def _text(text: str) -> ContentPart:
    return {"type": "input_text", "text": text}


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


def normalize_media_type(value: str, filename: str) -> str:
    media_type = value.split(";")[0].strip().lower()
    if media_type and media_type != "application/octet-stream":
        return media_type
    extension = filename.rsplit(".", 1)[-1].lower() if "." in filename else ""
    return _EXTENSIONS.get(extension, media_type or "application/octet-stream")
