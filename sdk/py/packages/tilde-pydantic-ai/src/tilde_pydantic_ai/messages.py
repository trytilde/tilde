"""Convert typed Tilde context into Pydantic AI model messages for ``message_history``.

User-role items become ``ModelRequest(parts=[UserPromptPart(...)])`` and the acting agent's own
messages become ``ModelResponse(parts=[TextPart(...)])``. Pydantic AI parts carry no id, so the
Tilde message id is stored in each message's application-level ``metadata`` (never sent to the
model) as ``{"tilde_message_id": id}`` and in the cached ``{"id", "role", "text"}`` representation.
"""

from __future__ import annotations

import inspect
import json
from collections.abc import Awaitable, Callable, Iterable
from dataclasses import dataclass
from typing import Any, Literal

from pydantic_ai.messages import (
    BinaryContent,
    FilePart,
    ModelMessage,
    ModelRequest,
    ModelResponse,
    TextPart,
    UserContent,
    UserPromptPart,
)

from tilde import (
    AgentContext,
    ContextMessage,
    ConversationMessage,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde_pydantic_ai.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    convert_attachment,
)

MESSAGE_ID_KEY = "tilde_message_id"
_MAX_BYTES = 1024 * 1024
_MAX_ITEMS = 100

Converted = ModelMessage | None
Handler = Callable[[Any], Converted | Awaitable[Converted]]


@dataclass(slots=True)
class MessageHandlers:
    """Explicit handlers take precedence over cached and default rendering; None omits an item."""

    message: Callable[[ConversationMessage], Converted | Awaitable[Converted]] | None = None
    objective: Callable[[ObjectiveMessage], Converted | Awaitable[Converted]] | None = None
    goal: Callable[[GoalMessage], Converted | Awaitable[Converted]] | None = None
    task: Callable[[TaskMessage], Converted | Awaitable[Converted]] | None = None


def text_message(id: str, role: Literal["user", "assistant"], text: str) -> ModelMessage:
    """A single-text message tagged with its Tilde id, the shape the converter caches."""
    return _build(id, role, [text])


def message_id(message: ModelMessage) -> str | None:
    """The Tilde message id a converted message came from, if it carries one."""
    return (message.metadata or {}).get(MESSAGE_ID_KEY)


async def convert_to_pydantic_ai_messages(
    messages: Iterable[ContextMessage],
    *,
    context: AgentContext | None = None,
    on_message: MessageHandlers | None = None,
    on_attachment: AttachmentHandler | None = None,
) -> list[ModelMessage]:
    """Convert typed SDK context into ``ModelMessage`` history for ``Agent.run``."""
    output: list[ModelMessage] = []
    batch: list[tuple[str, Any]] = []
    batch_bytes = 0

    async def flush() -> None:
        nonlocal batch_bytes
        if batch and context is not None:
            await context.cache_converted_messages(batch[:])
            batch.clear()
        batch_bytes = 0

    handlers = on_message or MessageHandlers()
    for item in messages:
        if (
            item.type == "message"
            and item.message.status != "complete"
            and handlers.message is None
        ):
            continue
        converted: ModelMessage | None
        cacheable = False
        if item.type == "objective":
            converted = (
                await _call(handlers.objective, item)
                if handlers.objective
                else text_message(item.id, "user", item.objective)
            )
        elif item.type == "goal":
            converted = (
                await _call(handlers.goal, item)
                if handlers.goal
                else text_message(
                    item.id, "user", f"Goal ({item.goal.status}): {item.goal.objective}"
                )
            )
        elif item.type == "task":
            if handlers.task:
                converted = await _call(handlers.task, item)
            else:
                text = f"Task ({item.task.status}): {item.task.title}"
                if item.task.blocked_reason:
                    text += f"\nBlocked: {item.task.blocked_reason}"
                converted = text_message(item.id, "user", text)
        elif handlers.message:
            converted = await _call(handlers.message, item)
        else:
            # File bytes are hydrated afresh through the scoped SDK, never replayed from the cache.
            converted = _hydrate(item) if not item.message.attachments else None
            if converted is None:
                converted = await _render(item, context, on_attachment)
                cacheable = item.message.status == "complete" and not item.message.attachments
        if converted is None:
            continue
        output.append(converted)
        # Only canonical messages can be cached by the runtime. Work state is always current,
        # and hydrated files must not duplicate large/sensitive attachment bytes in the cache.
        if context is None or not cacheable:
            continue
        entry = {"id": item.id, "role": item.role, "text": _text_of(converted)}
        size = len(json.dumps(entry).encode())
        if size > _MAX_BYTES:
            continue
        if len(batch) == _MAX_ITEMS or batch_bytes + size > _MAX_BYTES:
            await flush()
        batch.append((item.id, entry))
        batch_bytes += size
    await flush()
    return output


async def _call(handler: Handler, item: ContextMessage) -> Converted:
    result = handler(item)
    return await result if inspect.isawaitable(result) else result


async def _render(
    item: ConversationMessage,
    context: AgentContext | None,
    on_attachment: AttachmentHandler | None,
) -> ModelMessage | None:
    content: list[UserContent] = []
    if item.message.HasField("subject") and item.message.subject:
        content.append(f"Subject: {item.message.subject}")
    if item.message.text:
        content.append(item.message.text)
    for attachment in item.message.attachments:

        async def download(attachment=attachment):
            if context is None:
                raise RuntimeError(
                    "Attachment conversion requires context or a custom on_attachment handler"
                )
            return await context.attachments.download(attachment.id)

        result = (on_attachment or convert_attachment)(
            AttachmentConversion(message=item, attachment=attachment, download=download)
        )
        if inspect.isawaitable(result):
            result = await result
        if result is None:
            continue
        content.extend(result if isinstance(result, list) else [result])
    return _build(item.id, item.role, content) if content else None


def _build(id: str, role: Literal["user", "assistant"], content: list[UserContent]) -> ModelMessage:
    metadata = {MESSAGE_ID_KEY: id}
    if role == "assistant":
        parts = [
            FilePart(content=part) if isinstance(part, BinaryContent) else TextPart(str(part))
            for part in content
        ]
        return ModelResponse(parts=parts, metadata=metadata)
    prompt = content[0] if len(content) == 1 and isinstance(content[0], str) else content
    return ModelRequest(parts=[UserPromptPart(prompt)], metadata=metadata)


def _hydrate(item: ConversationMessage) -> ModelMessage | None:
    cached = item.cached_agent_representation
    if (
        not isinstance(cached, dict)
        or cached.get("id") != item.id
        or cached.get("role") != item.role
        or not isinstance(cached.get("text"), str)
    ):
        return None
    return text_message(item.id, item.role, cached["text"])


def _text_of(message: ModelMessage) -> str:
    texts: list[str] = []
    for part in message.parts:
        if isinstance(part, TextPart):
            texts.append(part.content)
        elif isinstance(part, UserPromptPart):
            content = [part.content] if isinstance(part.content, str) else part.content
            texts.extend(piece for piece in content if isinstance(piece, str))
    return "\n".join(texts)
