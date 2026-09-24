import base64

import pytest

from tilde import ConversationMessage, GoalMessage, ObjectiveMessage, TaskMessage
from tilde.types.v1.chat_pb2 import Attachment, Goal, Task
from tilde.types.v1.chat_pb2 import Message as ChatMessage
from tilde_crewai import MessageHandlers, convert_to_crewai_messages
from tilde_crewai.messages import MEDIA_REQUEST

PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZb8AAAAASUVORK5CYII="
)


def message(
    id: str, *, text: str = "Hello", status: str = "complete", cached=None, **fields
) -> ConversationMessage:
    return ConversationMessage(
        type="message",
        id=id,
        role="user",
        message=ChatMessage(id=id, status=status, text=text, **fields),
        cached_agent_representation=cached,
    )


class FakeContext:
    def __init__(self, files: dict[str, bytes]) -> None:
        self.files = files
        self.batches: list[list[tuple[str, object]]] = []
        self.attachments = self

    async def download(self, attachment_id: str):
        from tilde import DownloadedAttachment

        return DownloadedAttachment(
            attachment=Attachment(id=attachment_id), content=self.files[attachment_id]
        )

    async def cache_converted_messages(self, messages) -> None:
        self.batches.append(messages)


async def test_typed_callbacks_override_cached_rendering_and_may_omit_items():
    calls: list[str] = []
    items = [
        message("m", cached={"id": "m", "role": "user", "content": "cached"}),
        ObjectiveMessage(type="objective", id="o", objective="Objective"),
        GoalMessage(type="goal", id="g", goal=Goal(objective="Goal", status="active")),
        TaskMessage(type="task", id="t", task=Task(title="Task", status="working")),
    ]

    def on_message(item):
        calls.append(item.type)
        return {"role": "user", "content": "custom"}

    async def on_goal(item):
        calls.append(item.goal.objective)
        return {"role": "assistant", "content": "goal summary"}

    def omit(item):
        calls.append(getattr(item, "objective", None) or item.task.title)
        return None

    result = await convert_to_crewai_messages(
        items,
        on_message=MessageHandlers(message=on_message, objective=omit, goal=on_goal, task=omit),
    )
    assert calls == ["message", "Objective", "Goal", "Task"]
    assert [m["content"] for m in result] == ["custom", "goal summary"]
    assert result[1]["role"] == "assistant"


async def test_default_rendering_of_work_items_and_incomplete_messages():
    items = [
        message("draft", status="pending"),
        message("s", text="Body", subject="Topic"),
        ObjectiveMessage(type="objective", id="o", objective="Do the thing"),
        GoalMessage(type="goal", id="g", goal=Goal(objective="Ship", status="active")),
        TaskMessage(
            type="task", id="t", task=Task(title="Fix", status="blocked", blocked_reason="Waiting")
        ),
    ]
    result = await convert_to_crewai_messages(items)
    assert [m["content"] for m in result] == [
        "Subject: Topic\n\nBody",
        "Do the thing",
        "Goal (active): Ship",
        "Task (blocked): Fix\nBlocked: Waiting",
    ]
    assert all(m["role"] == "user" for m in result)


async def test_cache_hydration_validates_identity_role_and_bounded_batches_exclude_files():
    context = FakeContext({"file-id": b"file content"})
    source = message("good", cached={"id": "good", "role": "user", "content": "cached"})
    forged = message("forged", cached={"id": "forged", "role": "system", "content": "wrong role"})
    swapped = message("swapped", cached={"id": "other", "role": "user", "content": "wrong id"})
    file = message(
        "file",
        attachments=[Attachment(id="file-id", filename="notes.txt", media_type="text/plain")],
    )
    result = await convert_to_crewai_messages(
        [source, forged, swapped, file, *(message(f"m{i}") for i in range(101))],
        context=context,
    )
    assert [m["content"] for m in result[:3]] == ["cached", "Hello", "Hello"]
    assert result[3]["content"] == "Hello\n\nAttached file: notes.txt\nfile content"
    assert [len(batch) for batch in context.batches] == [100, 3]
    cached_ids = {entry[0] for batch in context.batches for entry in batch}
    assert "file" not in cached_ids and "good" not in cached_ids
    assert ("forged", {"id": "forged", "role": "user", "content": "Hello"}) in context.batches[0]


async def test_image_attachments_become_media_and_unknown_formats_are_described():
    context = FakeContext({"png": PNG})
    item = message(
        "m",
        text="",
        attachments=[
            Attachment(id="png", filename="picture.png", media_type="application/octet-stream"),
            Attachment(id="bin", filename="archive.zip", media_type="application/zip"),
        ],
    )
    result, request = await convert_to_crewai_messages([item], context=context)
    notice = (
        "Attached file: archive.zip (application/zip). "
        "A custom attachment handler is needed to read this format."
    )
    data_url = f"data:image/png;base64,{base64.b64encode(PNG).decode()}"
    assert result == {
        "role": "user",
        "content": [
            {"type": "text", "text": notice},
            {"type": "image_url", "image_url": {"url": data_url}},
        ],
    }
    # CrewAI flattens the last user message to text, so a text request follows the media.
    assert request == {"role": "user", "content": MEDIA_REQUEST}
    assert context.batches == []


async def test_custom_attachment_handler_receives_scoped_download():
    context = FakeContext({"png": PNG})
    item = message(
        "m", attachments=[Attachment(id="png", filename="picture.png", media_type="image/png")]
    )

    async def on_attachment(conversion):
        downloaded = await conversion.download()
        assert downloaded.attachment.id == conversion.attachment.id
        return f"Custom {conversion.attachment.filename}: {len(downloaded.content)} bytes"

    [result] = await convert_to_crewai_messages(
        [item], context=context, on_attachment=on_attachment
    )
    assert result == {"role": "user", "content": f"Hello\n\nCustom picture.png: {len(PNG)} bytes"}


async def test_attachments_are_not_silently_discarded_without_a_scoped_downloader():
    item = message(
        "m",
        text="",
        attachments=[Attachment(id="image", filename="picture.png", media_type="image/png")],
    )
    with pytest.raises(RuntimeError, match="requires context"):
        await convert_to_crewai_messages([item])
