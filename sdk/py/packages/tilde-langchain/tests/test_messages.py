import base64

import pytest
from langchain_core.messages import AIMessage, HumanMessage

from tilde import (
    ConversationMessage,
    DownloadedAttachment,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde.types.v1.chat_pb2 import Attachment, Goal, Message, Task
from tilde_langchain import convert_to_langchain_messages
from tilde_langchain.messages import MessageHandlers


def message(id: str, text: str = "Hello", **fields) -> ConversationMessage:
    return ConversationMessage(
        type="message",
        id=id,
        role="user",
        message=Message(id=id, status="complete", text=text, **fields),
    )


class FakeContext:
    def __init__(self) -> None:
        self.batches: list[list[tuple[str, object]]] = []
        self.downloads: list[str] = []

    class _Attachments:
        def __init__(self, outer: "FakeContext") -> None:
            self._outer = outer

        async def download(self, attachment_id: str) -> DownloadedAttachment:
            self._outer.downloads.append(attachment_id)
            content = b"\x89PNG-bytes" if attachment_id == "image-id" else b"file content"
            return DownloadedAttachment(attachment=Attachment(id=attachment_id), content=content)

    @property
    def attachments(self) -> "_Attachments":
        return FakeContext._Attachments(self)

    async def cache_converted_messages(self, messages) -> None:
        self.batches.append(messages)


async def test_typed_callbacks_override_cached_rendering_and_may_omit_items() -> None:
    source = message("m")
    source.cached_agent_representation = {"id": "m", "role": "user", "content": "cached"}
    calls: list[str] = []

    def on_message(item: ConversationMessage):
        calls.append(item.type)
        return HumanMessage(id=item.id, content="custom")

    async def on_objective(item: ObjectiveMessage):
        calls.append(item.objective)
        return None

    def on_goal(item: GoalMessage):
        calls.append(item.goal.objective)
        return AIMessage(id=item.id, content="goal summary")

    def on_task(item: TaskMessage):
        calls.append(item.task.title)
        return None

    result = await convert_to_langchain_messages(
        [
            source,
            ObjectiveMessage(type="objective", id="o", objective="Objective"),
            GoalMessage(type="goal", id="g", goal=Goal(objective="Goal", status="active")),
            TaskMessage(type="task", id="t", task=Task(title="Task", status="working")),
        ],
        on_message=MessageHandlers(
            message=on_message, objective=on_objective, goal=on_goal, task=on_task
        ),
    )
    assert calls == ["message", "Objective", "Goal", "Task"]
    assert [(m.id, m.content) for m in result] == [("m", "custom"), ("g", "goal summary")]


async def test_default_rendering_skips_incomplete_messages_and_renders_work_items() -> None:
    pending = message("p", "typing")
    pending.message.status = "pending"
    result = await convert_to_langchain_messages(
        [
            pending,
            message("s", "Body", subject="Invoice"),
            ObjectiveMessage(type="objective", id="o", objective="Do the thing"),
            GoalMessage(type="goal", id="g", goal=Goal(objective="Ship", status="active")),
            TaskMessage(
                type="task",
                id="t",
                task=Task(title="Deploy", status="blocked", blocked_reason="waiting"),
            ),
        ]
    )
    assert [m.id for m in result] == ["s", "o", "g", "t"]
    assert result[0].content == [
        {"type": "text", "text": "Subject: Invoice"},
        {"type": "text", "text": "Body"},
    ]
    assert result[1].content == "Do the thing"
    assert result[2].content == "Goal (active): Ship"
    assert result[3].content == "Task (blocked): Deploy\nBlocked: waiting"
    assert all(isinstance(m, HumanMessage) for m in result)


async def test_cache_hydration_validates_identity_and_batches_exclude_file_bytes() -> None:
    source = message("good")
    source.cached_agent_representation = {"id": "good", "role": "user", "content": "cached"}
    forged = message("forged")
    forged.cached_agent_representation = {"id": "forged", "role": "system", "content": "wrong"}
    stale_file = message("stale")
    stale_file.cached_agent_representation = {
        "id": "stale",
        "role": "user",
        "content": [{"type": "image", "base64": "AAAA", "mime_type": "image/png"}],
    }
    file = message(
        "file",
        attachments=[Attachment(id="file-id", filename="notes.txt", media_type="text/plain")],
    )
    image = message(
        "image",
        text="",
        attachments=[Attachment(id="image-id", filename="picture.png", media_type="image/png")],
    )
    context = FakeContext()
    result = await convert_to_langchain_messages(
        [source, forged, stale_file, file, image, *(message(f"m{i}") for i in range(101))],
        context=context,
    )
    assert result[0].content == "cached"
    assert result[1].content == [{"type": "text", "text": "Hello"}]
    assert result[2].content == [{"type": "text", "text": "Hello"}]
    assert result[3].content == [
        {"type": "text", "text": "Hello"},
        {"type": "text", "text": "Attached file: notes.txt\nfile content"},
    ]
    assert result[4].content == [
        {
            "type": "image",
            "mime_type": "image/png",
            "base64": base64.b64encode(b"\x89PNG-bytes").decode(),
        }
    ]
    assert context.downloads == ["file-id", "image-id"]
    assert [len(batch) for batch in context.batches] == [100, 3]
    cached_ids = {id for batch in context.batches for id, _ in batch}
    assert cached_ids.isdisjoint({"good", "file", "image"})
    assert {"forged", "stale"} <= cached_ids
    entry = dict(context.batches[0])["forged"]
    assert entry == {"id": "forged", "role": "user", "content": [{"type": "text", "text": "Hello"}]}


async def test_attachments_are_not_silently_discarded_without_a_scoped_downloader() -> None:
    item = message(
        "m",
        text="",
        attachments=[Attachment(id="image", filename="picture.png", media_type="image/png")],
    )
    with pytest.raises(RuntimeError, match="requires context"):
        await convert_to_langchain_messages([item])
    unsupported = message(
        "u",
        text="",
        attachments=[Attachment(id="bin", filename="data.bin", media_type="application/zip")],
    )
    result = await convert_to_langchain_messages([unsupported])
    assert "custom attachment handler" in result[0].content[0]["text"]


async def test_custom_attachment_handler_receives_the_scoped_download() -> None:
    context = FakeContext()
    item = message(
        "m",
        text="",
        attachments=[Attachment(id="file-id", filename="data.bin", media_type="application/zip")],
    )

    async def on_attachment(conversion):
        downloaded = await conversion.download()
        return {"type": "text", "text": f"decoded:{downloaded.content.decode()}"}

    result = await convert_to_langchain_messages(
        [item], context=context, on_attachment=on_attachment
    )
    assert result[0].content == [{"type": "text", "text": "decoded:file content"}]
    assert context.batches == []
