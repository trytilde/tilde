import json

import pytest
from pydantic_ai.messages import BinaryContent, ModelRequest, ModelResponse, UserPromptPart

from tilde import (
    ConversationMessage,
    DownloadedAttachment,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde.types.v1.chat_pb2 import Attachment, Goal, Message, Task
from tilde_pydantic_ai import convert_to_pydantic_ai_messages, message_id, text_message
from tilde_pydantic_ai.messages import MessageHandlers


def message(id: str, *, text: str = "Hello", cached=None, **fields) -> ConversationMessage:
    return ConversationMessage(
        type="message",
        id=id,
        role="user",
        message=Message(id=id, status="complete", text=text, **fields),
        cached_agent_representation=cached,
    )


class FakeContext:
    def __init__(self) -> None:
        self.batches: list[list[tuple[str, object]]] = []

    class attachments:
        @staticmethod
        async def download(id: str) -> DownloadedAttachment:
            content = b"file content" if id == "file-id" else b"\x89PNG\r\n\x1a\n"
            return DownloadedAttachment(attachment=Attachment(id=id), content=content)

    async def cache_converted_messages(self, messages):
        self.batches.append(messages)


def prompt_text(converted: ModelRequest) -> str:
    part = converted.parts[0]
    assert isinstance(part, UserPromptPart)
    return part.content if isinstance(part.content, str) else part.content[0]


async def test_typed_callbacks_override_cached_rendering_and_may_omit_items():
    calls = []
    items = [
        message("m", cached={"id": "m", "role": "user", "text": "cached"}),
        ObjectiveMessage(type="objective", id="o", objective="Objective"),
        GoalMessage(type="goal", id="g", goal=Goal(objective="Goal", status="active")),
        TaskMessage(type="task", id="t", task=Task(title="Task", status="working")),
    ]

    def on_message(item):
        calls.append(item.type)
        return text_message(item.id, "user", "custom")

    async def on_goal(item):
        calls.append(item.goal.objective)
        return text_message(item.id, "assistant", "goal summary")

    def omit(item):
        calls.append(getattr(item, "objective", None) or item.task.title)
        return None

    handlers = MessageHandlers(message=on_message, objective=omit, goal=on_goal, task=omit)
    result = await convert_to_pydantic_ai_messages(items, on_message=handlers)
    assert calls == ["message", "Objective", "Goal", "Task"]
    assert prompt_text(result[0]) == "custom"
    assert isinstance(result[1], ModelResponse)
    assert result[1].parts[0].content == "goal summary"
    assert [message_id(m) for m in result] == ["m", "g"]


async def test_default_rendering_of_work_items_and_subjects():
    items = [
        message("s", subject="Invoice", text="See attached"),
        ObjectiveMessage(type="objective", id="o", objective="Objective"),
        GoalMessage(type="goal", id="g", goal=Goal(objective="Ship", status="active")),
        TaskMessage(
            type="task", id="t", task=Task(title="Deploy", status="blocked", blocked_reason="CI")
        ),
        ConversationMessage(
            type="message",
            id="p",
            role="user",
            message=Message(id="p", status="pending", text="draft"),
        ),
    ]
    result = await convert_to_pydantic_ai_messages(items)
    assert result[0].parts[0].content == ["Subject: Invoice", "See attached"]
    assert [prompt_text(m) for m in result[1:]] == [
        "Objective",
        "Goal (active): Ship",
        "Task (blocked): Deploy\nBlocked: CI",
    ]
    assert len(result) == 4  # The pending draft is skipped without a custom handler.


async def test_cache_hydration_validates_identity_and_role_and_batches_exclude_file_bytes():
    good = message("good", cached={"id": "good", "role": "user", "text": "cached"})
    forged = message("forged", cached={"id": "forged", "role": "system", "text": "wrong role"})
    stale = message("stale", cached={"id": "other", "role": "user", "text": "wrong id"})
    text_file = message(
        "file",
        attachments=[Attachment(id="file-id", filename="notes.txt", media_type="text/plain")],
    )
    image = message(
        "image",
        text="",
        attachments=[Attachment(id="png-id", filename="picture.png", media_type="image/png")],
    )
    context = FakeContext()
    result = await convert_to_pydantic_ai_messages(
        [good, forged, stale, text_file, image, *(message(f"m{i}") for i in range(101))],
        context=context,
    )
    assert prompt_text(result[0]) == "cached"
    assert prompt_text(result[1]) == "Hello"
    assert prompt_text(result[2]) == "Hello"
    assert result[3].parts[0].content == ["Hello", "Attached file: notes.txt\nfile content"]
    binary = result[4].parts[0].content[0]
    assert isinstance(binary, BinaryContent)
    assert binary.media_type == "image/png" and binary.data.startswith(b"\x89PNG")
    assert [len(batch) for batch in context.batches] == [100, 3]
    cached_ids = [id for batch in context.batches for id, _ in batch]
    assert not {"good", "file", "image"} & set(cached_ids)
    assert {"forged", "stale"} <= set(cached_ids)
    for _, entry in context.batches[0]:
        json.dumps(entry)  # Plain JSON without binary content.
        assert set(entry) == {"id", "role", "text"}


async def test_large_entries_are_skipped_and_bytes_bound_batches():
    context = FakeContext()
    big = "x" * (1024 * 1024)
    half = "y" * (600 * 1024)
    await convert_to_pydantic_ai_messages(
        [message("big", text=big), message("a", text=half), message("b", text=half)],
        context=context,
    )
    assert [[id for id, _ in batch] for batch in context.batches] == [["a"], ["b"]]


async def test_octet_stream_falls_back_to_extension_and_unknown_types_are_described():
    context = FakeContext()
    result = await convert_to_pydantic_ai_messages(
        [
            message(
                "m",
                text="",
                attachments=[
                    Attachment(
                        id="png-id", filename="shot.PNG", media_type="application/octet-stream"
                    ),
                    Attachment(id="zip", filename="a.zip", media_type="application/zip"),
                ],
            )
        ],
        context=context,
    )
    content = result[0].parts[0].content
    assert isinstance(content[0], BinaryContent) and content[0].media_type == "image/png"
    assert content[1].startswith("Attached file: a.zip (application/zip).")
    assert context.batches == []


async def test_attachments_are_not_silently_discarded_without_a_scoped_downloader():
    item = message(
        "m",
        text="",
        attachments=[Attachment(id="image", filename="picture.png", media_type="image/png")],
    )
    with pytest.raises(RuntimeError, match="requires context"):
        await convert_to_pydantic_ai_messages([item])

    async def on_attachment(input):
        return f"parsed {input.attachment.filename}"

    result = await convert_to_pydantic_ai_messages([item], on_attachment=on_attachment)
    assert prompt_text(result[0]) == "parsed picture.png"
