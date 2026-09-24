"""Convert typed Tilde context into LangChain messages."""

from __future__ import annotations

import inspect
import json
from collections.abc import Awaitable, Callable, Iterable
from dataclasses import dataclass
from typing import Any, Protocol

from langchain_core.messages import AIMessage, BaseMessage, HumanMessage

from tilde import (
    ContextMessage,
    ConversationMessage,
    DownloadedAttachment,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde_langchain.attachments import AttachmentConversion, AttachmentHandler, convert_attachment

Converted = BaseMessage | None
_Handler = Callable[[Any], Converted | Awaitable[Converted]]


@dataclass(slots=True)
class MessageHandlers:
    """Explicit handlers take precedence over cached and default rendering; None omits an item."""

    message: Callable[[ConversationMessage], Converted | Awaitable[Converted]] | None = None
    objective: Callable[[ObjectiveMessage], Converted | Awaitable[Converted]] | None = None
    goal: Callable[[GoalMessage], Converted | Awaitable[Converted]] | None = None
    task: Callable[[TaskMessage], Converted | Awaitable[Converted]] | None = None


class _Attachments(Protocol):
    async def download(self, attachment_id: str) -> DownloadedAttachment: ...


class ConversionContext(Protocol):
    """The parts of ``AgentContext`` the converter uses; a test double only needs these."""

    attachments: _Attachments

    async def cache_converted_messages(self, messages: list[tuple[str, Any]]) -> None: ...


_BATCH_ITEMS = 100
_BATCH_BYTES = 1024 * 1024


async def convert_to_langchain_messages(
    messages: Iterable[ContextMessage],
    *,
    context: ConversionContext | None = None,
    on_message: MessageHandlers | None = None,
    on_attachment: AttachmentHandler | None = None,
) -> list[BaseMessage]:
    """Convert typed SDK context into ``HumanMessage``/``AIMessage`` items, ready for an agent.

    Each result carries the Tilde message id. Incomplete conversation messages are skipped
    unless a custom ``message`` handler is supplied. Completed, attachment-free conversions are
    cached through ``context.cache_converted_messages`` in bounded batches; attachments are
    hydrated afresh on every run and never stored in the cache.
    """
    output: list[BaseMessage] = []
    batch: list[tuple[str, Any]] = []
    batch_bytes = 0

    async def flush() -> None:
        nonlocal batch_bytes
        if batch and context is not None:
            await context.cache_converted_messages(batch[:])
        batch.clear()
        batch_bytes = 0

    for item in messages:
        handler: _Handler | None
        converted: Converted
        cacheable = False
        if isinstance(item, ObjectiveMessage):
            handler = on_message.objective if on_message else None
            converted = (
                await _call(handler, item)
                if handler
                else HumanMessage(id=item.id, content=item.objective)
            )
        elif isinstance(item, GoalMessage):
            handler = on_message.goal if on_message else None
            converted = (
                await _call(handler, item)
                if handler
                else HumanMessage(
                    id=item.id, content=f"Goal ({item.goal.status}): {item.goal.objective}"
                )
            )
        elif isinstance(item, TaskMessage):
            handler = on_message.task if on_message else None
            if handler:
                converted = await _call(handler, item)
            else:
                text = f"Task ({item.task.status}): {item.task.title}"
                if item.task.blocked_reason:
                    text += f"\nBlocked: {item.task.blocked_reason}"
                converted = HumanMessage(id=item.id, content=text)
        else:
            handler = on_message.message if on_message else None
            if handler:
                converted = await _call(handler, item)
            elif item.message.status != "complete":
                continue
            else:
                # File bytes are hydrated afresh through the scoped SDK, never from the cache.
                hydrated = _hydrate(item) if not item.message.attachments else None
                converted = hydrated or await _render(item, context, on_attachment)
                # Only canonical, attachment-free chat records are cached by the runtime; work
                # state stays a live projection and file bytes never duplicate into the cache.
                cacheable = hydrated is None and not item.message.attachments
        if converted is None:
            continue
        output.append(converted)
        if cacheable and context is not None:
            entry = {"id": converted.id, "role": _role(converted), "content": converted.content}
            serialized = json.dumps(entry)
            size = len(serialized.encode())
            if size > _BATCH_BYTES:
                continue
            if len(batch) == _BATCH_ITEMS or batch_bytes + size > _BATCH_BYTES:
                await flush()
            batch.append((item.id, json.loads(serialized)))
            batch_bytes += size
    await flush()
    return output


async def _call(handler: _Handler, item: ContextMessage) -> Converted:
    result = handler(item)
    if inspect.isawaitable(result):
        result = await result
    return result


async def _render(
    item: ConversationMessage,
    context: ConversionContext | None,
    on_attachment: AttachmentHandler | None,
) -> Converted:
    blocks: list[dict[str, Any]] = []
    if item.message.subject:
        blocks.append({"type": "text", "text": f"Subject: {item.message.subject}"})
    if item.message.text:
        blocks.append({"type": "text", "text": item.message.text})
    for attachment in item.message.attachments:

        def download(attachment_id: str = attachment.id) -> Awaitable[DownloadedAttachment]:
            if context is None:
                raise RuntimeError(
                    "Attachment conversion requires context or a custom on_attachment handler"
                )
            return context.attachments.download(attachment_id)

        result = (on_attachment or convert_attachment)(
            AttachmentConversion(message=item, attachment=attachment, download=download)
        )
        if inspect.isawaitable(result):
            result = await result
        if result is None:
            continue
        blocks.extend(result if isinstance(result, list) else [result])
    if not blocks:
        return None
    return _message(item.role, item.id, blocks)


def _hydrate(item: ConversationMessage) -> Converted:
    cached = item.cached_agent_representation
    if (
        not isinstance(cached, dict)
        or cached.get("id") != item.id
        or cached.get("role") != item.role
    ):
        return None
    content = cached.get("content")
    if isinstance(content, str):
        return _message(item.role, item.id, content)
    if not isinstance(content, list) or not all(isinstance(block, dict) for block in content):
        return None
    if any(block.get("type") in ("image", "file") for block in content):
        return None
    return _message(item.role, item.id, content)


def _message(role: str, id: str, content: str | list[dict[str, Any]]) -> BaseMessage:
    if role == "assistant":
        return AIMessage(id=id, content=content)
    return HumanMessage(id=id, content=content)


def _role(message: BaseMessage) -> str:
    return "assistant" if isinstance(message, AIMessage) else "user"
