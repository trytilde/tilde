"""Convert typed Tilde context into Agno messages.

Pass the result as ``await agent.arun(input=messages)``: Agno appends a ``list[Message]`` input
verbatim after its system message, so roles and media survive and no separate "latest text"
needs to be extracted. Keep ``add_history_to_context`` off; Tilde owns the history.
"""

from __future__ import annotations

import inspect
import json
from collections.abc import Awaitable, Callable, Iterable
from dataclasses import dataclass
from typing import Any

from agno.media import File, Image
from agno.models.message import Message

from tilde import (
    AgentContext,
    ContextMessage,
    ConversationMessage,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde_agno.attachments import AttachmentConversion, AttachmentHandler, convert_attachment

Converted = Message | None | Awaitable[Message | None]


@dataclass(slots=True)
class MessageHandlers:
    """Explicit handlers take precedence over cached and default rendering; None omits an item."""

    message: Callable[[ConversationMessage], Converted] | None = None
    objective: Callable[[ObjectiveMessage], Converted] | None = None
    goal: Callable[[GoalMessage], Converted] | None = None
    task: Callable[[TaskMessage], Converted] | None = None


_BATCH_ITEMS = 100
_BATCH_BYTES = 1024 * 1024


async def convert_to_agno_messages(
    messages: Iterable[ContextMessage],
    *,
    context: AgentContext | None = None,
    on_message: MessageHandlers | None = None,
    on_attachment: AttachmentHandler | None = None,
) -> list[Message]:
    """Convert typed SDK context into ``agno.models.message.Message`` values."""
    output: list[Message] = []
    handlers = on_message or MessageHandlers()
    cache: list[tuple[str, Any]] = []
    cache_bytes = 0

    async def flush() -> None:
        nonlocal cache_bytes
        if cache and context is not None:
            await context.cache_converted_messages(cache[:])
            cache.clear()
        cache_bytes = 0

    for item in messages:
        hydrated = False
        if isinstance(item, ObjectiveMessage):
            converted = (
                await _call(handlers.objective, item)
                if handlers.objective
                else Message(id=item.id, role="user", content=item.objective)
            )
        elif isinstance(item, GoalMessage):
            converted = (
                await _call(handlers.goal, item)
                if handlers.goal
                else Message(
                    id=item.id,
                    role="user",
                    content=f"Goal ({item.goal.status}): {item.goal.objective}",
                )
            )
        elif isinstance(item, TaskMessage):
            if handlers.task:
                converted = await _call(handlers.task, item)
            else:
                text = f"Task ({item.task.status}): {item.task.title}"
                if item.task.blocked_reason:
                    text += f"\nBlocked: {item.task.blocked_reason}"
                converted = Message(id=item.id, role="user", content=text)
        else:
            if item.message.status != "complete" and handlers.message is None:
                continue
            if handlers.message:
                converted = await _call(handlers.message, item)
            else:
                # File bytes are hydrated afresh through the scoped SDK, never from the cache.
                converted = _hydrate(item) if not item.message.attachments else None
                hydrated = converted is not None
                if converted is None:
                    converted = await _render(item, context, on_attachment)
        if converted is None:
            continue
        output.append(converted)
        # Only canonical, text-only conversation messages are cached. Work state is always
        # current, and hydrated files must not duplicate attachment bytes in the cache.
        if (
            context is not None
            and isinstance(item, ConversationMessage)
            and item.message.status == "complete"
            and not hydrated
            and not item.message.attachments
            and isinstance(converted.content, str)
            and not (converted.images or converted.files or converted.audio or converted.videos)
        ):
            entry = {"id": converted.id, "role": converted.role, "content": converted.content}
            size = len(json.dumps(entry).encode())
            if size > _BATCH_BYTES:
                continue
            if len(cache) == _BATCH_ITEMS or cache_bytes + size > _BATCH_BYTES:
                await flush()
            cache.append((item.id, entry))
            cache_bytes += size
    await flush()
    return output


async def _call(handler: Callable[[Any], Any], value: Any) -> Any:
    result = handler(value)
    return await result if inspect.isawaitable(result) else result


def _hydrate(item: ConversationMessage) -> Message | None:
    cached = item.cached_agent_representation
    if not isinstance(cached, dict):
        return None
    content = cached.get("content")
    if cached.get("id") != item.id or cached.get("role") != item.role:
        return None
    if not isinstance(content, str):
        return None
    return Message(id=item.id, role=item.role, content=content)


def _downloader(context: AgentContext | None, attachment_id: str):
    def download():
        if context is None:
            raise RuntimeError(
                "Attachment conversion requires context or a custom on_attachment handler"
            )
        return context.attachments.download(attachment_id)

    return download


async def _render(
    item: ConversationMessage, context: AgentContext | None, on_attachment: AttachmentHandler | None
) -> Message | None:
    texts: list[str] = []
    images: list[Image] = []
    files: list[File] = []
    if item.message.subject:
        texts.append(f"Subject: {item.message.subject}")
    if item.message.text:
        texts.append(item.message.text)
    handler = on_attachment or convert_attachment
    for attachment in item.message.attachments:
        conversion = AttachmentConversion(
            message=item, attachment=attachment, download=_downloader(context, attachment.id)
        )
        result = await _call(handler, conversion)
        for part in result if isinstance(result, list) else [result]:
            if isinstance(part, str):
                texts.append(part)
            elif isinstance(part, Image):
                images.append(part)
            elif isinstance(part, File):
                files.append(part)
    if not texts and not images and not files:
        return None
    if not texts:
        # Providers reject empty text beside media; name the files instead of sending "".
        texts.append("Attached: " + ", ".join(a.filename for a in item.message.attachments))
    return Message(
        id=item.id,
        role=item.role,
        content="\n\n".join(texts),
        images=images or None,
        files=files or None,
    )
