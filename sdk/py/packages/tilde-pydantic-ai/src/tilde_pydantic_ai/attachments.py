"""Attachment hydration: real file bytes for the model, never private URLs or credentials."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from dataclasses import dataclass

from pydantic_ai.messages import BinaryContent, UserContent

from tilde import ConversationMessage, DownloadedAttachment
from tilde.types.v1.chat_pb2 import Attachment


@dataclass(slots=True)
class AttachmentConversion:
    message: ConversationMessage
    attachment: Attachment
    download: Callable[[], Awaitable[DownloadedAttachment]]
    """Uses the current invocation's authenticated, thread-scoped attachment API."""


AttachmentResult = UserContent | list[UserContent] | None
AttachmentHandler = Callable[[AttachmentConversion], AttachmentResult | Awaitable[AttachmentResult]]

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


async def convert_attachment(input: AttachmentConversion) -> AttachmentResult:
    """Hydrate images/PDFs as binary content and textual files as actual text, not filenames."""
    attachment = input.attachment
    media_type = normalize_media_type(attachment.media_type, attachment.filename)
    text = media_type.startswith("text/") or media_type in _TEXT_TYPES
    if not text and media_type not in _FILE_TYPES:
        return (
            f"Attached file: {attachment.filename} ({media_type}). "
            "A custom attachment handler is needed to read this format."
        )
    result = await input.download()
    if result.attachment.id != attachment.id:
        raise RuntimeError("Attachment download returned a different file")
    if text:
        return f"Attached file: {attachment.filename}\n{result.content.decode('utf-8', 'replace')}"
    return BinaryContent(result.content, media_type=media_type, identifier=attachment.filename)


def normalize_media_type(value: str, filename: str) -> str:
    kind = value.split(";")[0].strip().lower()
    if kind and kind != "application/octet-stream":
        return kind
    extension = filename.rsplit(".", 1)[-1].lower() if "." in filename else ""
    return _EXTENSIONS.get(extension, kind or "application/octet-stream")
