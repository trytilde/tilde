"""Typed Tilde context to Responses API input items for ``agents.Runner.run``."""

from __future__ import annotations

import inspect
import json
from collections.abc import Awaitable, Callable, Iterable
from dataclasses import dataclass
from typing import Any

from agents import TResponseInputItem

from tilde import (
    AgentContext,
    ContextMessage,
    ConversationMessage,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde_openai_agents.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    convert_attachment,
)

Converted = TResponseInputItem | None
Handler = Callable[[Any], Converted | Awaitable[Converted]]

_MAX_ENTRY_BYTES = 1024 * 1024
_MAX_BATCH_ITEMS = 100
_CACHED_PART_TYPES = {"input_text", "output_text"}


@dataclass(slots=True)
class MessageHandlers:
    """Explicit handlers take precedence over cached and default conversion; None omits an item."""

    message: Callable[[ConversationMessage], Converted | Awaitable[Converted]] | None = None
    objective: Callable[[ObjectiveMessage], Converted | Awaitable[Converted]] | None = None
    goal: Callable[[GoalMessage], Converted | Awaitable[Converted]] | None = None
    task: Callable[[TaskMessage], Converted | Awaitable[Converted]] | None = None


async def convert_to_openai_agents_messages(
    messages: Iterable[ContextMessage],
    *,
    context: AgentContext | None = None,
    on_message: MessageHandlers | None = None,
    on_attachment: AttachmentHandler | None = None,
) -> list[TResponseInputItem]:
    """Convert typed SDK context into Responses input items.

    Returned items carry no ``id``: the Responses API rejects ids it did not issue. The Tilde
    message id lives only in the cached representation, where it validates a hydration.
    """
    handlers = on_message or MessageHandlers()
    output: list[TResponseInputItem] = []
    batch: list[tuple[str, Any]] = []
    batch_bytes = 0

    async def flush() -> None:
        nonlocal batch_bytes
        if batch and context is not None:
            await context.cache_converted_messages(batch[:])
        batch.clear()
        batch_bytes = 0

    for item in messages:
        hydrated = False
        if isinstance(item, ObjectiveMessage):
            converted = await _convert(handlers.objective, item, "user", item.objective)
        elif isinstance(item, GoalMessage):
            converted = await _convert(
                handlers.goal, item, "user", f"Goal ({item.goal.status}): {item.goal.objective}"
            )
        elif isinstance(item, TaskMessage):
            text = f"Task ({item.task.status}): {item.task.title}"
            if item.task.blocked_reason:
                text += f"\nBlocked: {item.task.blocked_reason}"
            converted = await _convert(handlers.task, item, "user", text)
        elif handlers.message is not None:
            converted = await _call(handlers.message, item)
        elif item.message.status != "complete":
            continue
        else:
            # File bytes are hydrated afresh through the scoped SDK, never replayed from the cache.
            converted = _hydrate(item) if not item.message.attachments else None
            hydrated = converted is not None
            if converted is None:
                converted = await _render(item, context, on_attachment)
        if converted is None:
            continue
        output.append(converted)
        # Only canonical, complete, attachment-free messages are cached; work state stays live.
        if (
            context is None
            or hydrated
            or not isinstance(item, ConversationMessage)
            or item.message.status != "complete"
            or item.message.attachments
        ):
            continue
        entry = {"id": item.id, **converted}
        size = len(json.dumps(entry).encode())
        if size > _MAX_ENTRY_BYTES:
            continue
        if len(batch) == _MAX_BATCH_ITEMS or batch_bytes + size > _MAX_ENTRY_BYTES:
            await flush()
        batch.append((item.id, json.loads(json.dumps(entry))))
        batch_bytes += size
    await flush()
    return output


async def _convert(
    handler: Handler | None, item: ContextMessage, role: str, text: str
) -> Converted:
    """A supplied handler decides (None omits); otherwise render the default text item."""
    return await _call(handler, item) if handler is not None else _text_item(role, text)


async def _call(handler: Handler, item: ContextMessage) -> Converted:
    result = handler(item)
    if inspect.isawaitable(result):
        result = await result
    return result


def _text_item(role: str, text: str) -> TResponseInputItem:
    part_type = "output_text" if role == "assistant" else "input_text"
    return {"role": role, "content": [{"type": part_type, "text": text}]}  # type: ignore[return-value]


async def _render(
    item: ConversationMessage, context: AgentContext | None, on_attachment: AttachmentHandler | None
) -> Converted:
    parts: list[dict[str, Any]] = []
    if item.message.HasField("subject") and item.message.subject:
        parts.append({"type": "input_text", "text": f"Subject: {item.message.subject}"})
    if item.message.text:
        parts.append({"type": "input_text", "text": item.message.text})
    for attachment in item.message.attachments:

        async def download(attachment_id: str = attachment.id):
            if context is None:
                raise RuntimeError(
                    "Attachment conversion requires context or a custom on_attachment handler"
                )
            return await context.attachments.download(attachment_id)

        converted = (on_attachment or convert_attachment)(
            AttachmentConversion(message=item, attachment=attachment, download=download)
        )
        if inspect.isawaitable(converted):
            converted = await converted
        if converted is None:
            continue
        parts.extend(converted if isinstance(converted, list) else [converted])
    if not parts:
        return None
    if item.role == "assistant":
        parts = [
            {**part, "type": "output_text"} if part.get("type") == "input_text" else part
            for part in parts
        ]
    return {"role": item.role, "content": parts}  # type: ignore[return-value]


def _hydrate(item: ConversationMessage) -> Converted:
    """Reuse a cached rendering only when its id, role and text-only content check out."""
    cached = item.cached_agent_representation
    if (
        not isinstance(cached, dict)
        or cached.get("id") != item.id
        or cached.get("role") != item.role
    ):
        return None
    content = cached.get("content")
    if not isinstance(content, list) or not content:
        return None
    for part in content:
        if (
            not isinstance(part, dict)
            or part.get("type") not in _CACHED_PART_TYPES
            or not isinstance(part.get("text"), str)
        ):
            return None
    return {"role": item.role, "content": content}  # type: ignore[return-value]
