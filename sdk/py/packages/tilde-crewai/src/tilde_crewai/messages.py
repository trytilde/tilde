"""Convert typed Tilde context into CrewAI messages.

Pass the result as ``await agent.kickoff_async(messages)``. CrewAI takes OpenAI-style
``{"role", "content"}`` dicts (``crewai.utilities.types.LLMMessage``): every message keeps its
role as conversation history, except the last ``user`` message, which CrewAI collapses to text
and promotes into its task prompt. Content parts on that one message would be lost, so when it
carries media a short text request is appended to take its place as the promoted turn.
"""

from __future__ import annotations

import inspect
import json
from collections.abc import Awaitable, Callable, Iterable
from dataclasses import dataclass
from typing import Any

from crewai.utilities.types import LLMMessage

from tilde import (
    AgentContext,
    ContextMessage,
    ConversationMessage,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde_crewai.attachments import AttachmentConversion, AttachmentHandler, convert_attachment

Converted = LLMMessage | None | Awaitable[LLMMessage | None]

MEDIA_REQUEST = "Respond to the latest message above, using its attachments."


@dataclass(slots=True)
class MessageHandlers:
    """Explicit handlers take precedence over cached and default rendering; None omits an item."""

    message: Callable[[ConversationMessage], Converted] | None = None
    objective: Callable[[ObjectiveMessage], Converted] | None = None
    goal: Callable[[GoalMessage], Converted] | None = None
    task: Callable[[TaskMessage], Converted] | None = None


_BATCH_ITEMS = 100
_BATCH_BYTES = 1024 * 1024


async def convert_to_crewai_messages(
    messages: Iterable[ContextMessage],
    *,
    context: AgentContext | None = None,
    on_message: MessageHandlers | None = None,
    on_attachment: AttachmentHandler | None = None,
) -> list[LLMMessage]:
    """Convert typed SDK context into the message dicts ``Agent.kickoff_async`` accepts."""
    output: list[LLMMessage] = []
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
                else _user(item.objective)
            )
        elif isinstance(item, GoalMessage):
            converted = (
                await _call(handlers.goal, item)
                if handlers.goal
                else _user(f"Goal ({item.goal.status}): {item.goal.objective}")
            )
        elif isinstance(item, TaskMessage):
            if handlers.task:
                converted = await _call(handlers.task, item)
            else:
                text = f"Task ({item.task.status}): {item.task.title}"
                if item.task.blocked_reason:
                    text += f"\nBlocked: {item.task.blocked_reason}"
                converted = _user(text)
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
            and isinstance(converted.get("content"), str)
        ):
            entry = {"id": item.id, "role": converted["role"], "content": converted["content"]}
            size = len(json.dumps(entry).encode())
            if size > _BATCH_BYTES:
                continue
            if len(cache) == _BATCH_ITEMS or cache_bytes + size > _BATCH_BYTES:
                await flush()
            cache.append((item.id, entry))
            cache_bytes += size
    await flush()
    # CrewAI flattens the last user message to text; keep its media in history instead.
    last_user = next((m for m in reversed(output) if m["role"] == "user"), None)
    if last_user is not None and isinstance(last_user["content"], list):
        output.append(_user(MEDIA_REQUEST))
    return output


def _user(text: str) -> LLMMessage:
    return {"role": "user", "content": text}


async def _call(handler: Callable[[Any], Any], value: Any) -> Any:
    result = handler(value)
    return await result if inspect.isawaitable(result) else result


def _hydrate(item: ConversationMessage) -> LLMMessage | None:
    cached = item.cached_agent_representation
    if not isinstance(cached, dict):
        return None
    content = cached.get("content")
    if cached.get("id") != item.id or cached.get("role") != item.role:
        return None
    if not isinstance(content, str):
        return None
    return {"role": item.role, "content": content}


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
) -> LLMMessage | None:
    texts: list[str] = []
    parts: list[dict[str, Any]] = []
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
            elif isinstance(part, dict):
                parts.append(part)
    if not texts and not parts:
        return None
    if not parts:
        return {"role": item.role, "content": "\n\n".join(texts)}
    if not texts:
        # Providers reject empty text beside media; name the files instead of sending "".
        texts.append("Attached: " + ", ".join(a.filename for a in item.message.attachments))
    return {"role": item.role, "content": [{"type": "text", "text": "\n\n".join(texts)}, *parts]}
