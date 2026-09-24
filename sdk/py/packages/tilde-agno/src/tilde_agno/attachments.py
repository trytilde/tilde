"""Default attachment hydration: images and PDFs become Agno media, text files become text."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from dataclasses import dataclass

from agno.media import File, Image

from tilde import ConversationMessage, DownloadedAttachment
from tilde.types.v1.chat_pb2 import Attachment

AttachmentPart = str | Image | File
AttachmentResult = AttachmentPart | list[AttachmentPart] | None


@dataclass(slots=True)
class AttachmentConversion:
    message: ConversationMessage
    attachment: Attachment
    # Uses the current invocation's authenticated, thread-scoped attachment API.
    download: Callable[[], Awaitable[DownloadedAttachment]]


AttachmentHandler = Callable[[AttachmentConversion], AttachmentResult | Awaitable[AttachmentResult]]

_TEXT_TYPES = {"application/json", "application/xml"}
_IMAGE_TYPES = {"image/jpeg", "image/png", "image/gif", "image/webp"}
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
    """Hydrate images/PDFs as Agno media and textual files as their actual text, not filenames."""
    attachment = input.attachment
    media_type = normalize_media_type(attachment.media_type, attachment.filename)
    text = media_type.startswith("text/") or media_type in _TEXT_TYPES
    image = media_type in _IMAGE_TYPES
    if not text and not image and media_type != "application/pdf":
        return (
            f"Attached file: {attachment.filename} ({media_type}). "
            "A custom attachment handler is needed to read this format."
        )
    result = await input.download()
    if result.attachment.id != attachment.id:
        raise RuntimeError("Attachment download returned a different file")
    if text:
        return f"Attached file: {attachment.filename}\n{result.content.decode('utf-8', 'replace')}"
    if image:
        return Image(
            content=result.content, mime_type=media_type, format=media_type.removeprefix("image/")
        )
    return File(
        content=result.content, mime_type=media_type, filename=attachment.filename, format="pdf"
    )


def normalize_media_type(value: str, filename: str) -> str:
    media_type = value.split(";")[0].strip().lower()
    if media_type and media_type != "application/octet-stream":
        return media_type
    extension = filename.rsplit(".", 1)[-1].lower() if "." in filename else ""
    return _EXTENSIONS.get(extension, media_type or "application/octet-stream")
