import base64
from dataclasses import dataclass, field
from typing import Any

import pytest

from tilde import (
    ConversationMessage,
    DownloadedAttachment,
    GoalMessage,
    ObjectiveMessage,
    TaskMessage,
)
from tilde.types.v1.chat_pb2 import Attachment, Goal, Message, Task
from tilde_openai_agents import MessageHandlers, convert_to_openai_agents_messages

PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZb8AAAAASUVORK5CYII="
)
FILES = {"file-id": b"file content", "image": PNG}


def message(
    id: str,
    *,
    role: str = "user",
    text: str = "Hello",
    attachments: list[Attachment] | None = None,
    cached: Any = None,
    subject: str | None = None,
) -> ConversationMessage:
    proto = Message(id=id, status="complete", text=text, attachments=attachments or [])
    if subject is not None:
        proto.subject = subject
    return ConversationMessage(
        type="message", id=id, role=role, message=proto, cached_agent_representation=cached
    )


def cached(id: str, role: str = "user", text: str = "cached") -> dict[str, Any]:
    return {"id": id, "role": role, "content": [{"type": "input_text", "text": text}]}


class _Attachments:
    async def download(self, attachment_id: str) -> DownloadedAttachment:
        return DownloadedAttachment(
            attachment=Attachment(id=attachment_id), content=FILES[attachment_id]
        )


@dataclass
class FakeContext:
    attachments: _Attachments = field(default_factory=_Attachments)
    batches: list[list[tuple[str, Any]]] = field(default_factory=list)

    async def cache_converted_messages(self, messages: list[tuple[str, Any]]) -> None:
        self.batches.append(messages)


def texts(items: list[Any]) -> list[str]:
    return [part["text"] for item in items for part in item["content"]]


async def test_typed_callbacks_override_cached_rendering_and_may_omit_items() -> None:
    calls: list[str] = []

    def on_message(item: ConversationMessage):
        calls.append(item.type)
        return {"role": "user", "content": [{"type": "input_text", "text": "custom"}]}

    async def on_goal(item: GoalMessage):
        calls.append(item.goal.objective)
        return {"role": "assistant", "content": [{"type": "output_text", "text": "goal summary"}]}

    def omit(item: Any):
        calls.append(item.objective if isinstance(item, ObjectiveMessage) else item.task.title)
        return None

    result = await convert_to_openai_agents_messages(
        [
            message("m", cached=cached("m")),
            ObjectiveMessage(type="objective", id="o", objective="Objective"),
            GoalMessage(type="goal", id="g", goal=Goal(objective="Goal", status="active")),
            TaskMessage(type="task", id="t", task=Task(title="Task", status="working")),
        ],
        on_message=MessageHandlers(message=on_message, objective=omit, goal=on_goal, task=omit),
    )
    assert calls == ["message", "Objective", "Goal", "Task"]
    assert texts(result) == ["custom", "goal summary"]
    assert [item["role"] for item in result] == ["user", "assistant"]


async def test_default_rendering_of_work_items_subjects_and_assistant_messages() -> None:
    result = await convert_to_openai_agents_messages(
        [
            message("m", subject="Invoice", text="See attached"),
            message("a", role="assistant", text="On it"),
            ObjectiveMessage(type="objective", id="o", objective="Objective"),
            GoalMessage(type="goal", id="g", goal=Goal(objective="Ship", status="active")),
            TaskMessage(
                type="task",
                id="t",
                task=Task(title="Review", status="blocked", blocked_reason="waiting"),
            ),
        ]
    )
    assert result[0] == {
        "role": "user",
        "content": [
            {"type": "input_text", "text": "Subject: Invoice"},
            {"type": "input_text", "text": "See attached"},
        ],
    }
    assert result[1] == {"role": "assistant", "content": [{"type": "output_text", "text": "On it"}]}
    assert texts(result[2:]) == [
        "Objective",
        "Goal (active): Ship",
        "Task (blocked): Review\nBlocked: waiting",
    ]
    assert all("id" not in item for item in result)


async def test_cache_hydration_validates_identity_and_role_and_batches_exclude_file_bytes() -> None:
    context = FakeContext()
    file = message(
        "file",
        attachments=[Attachment(id="file-id", filename="notes.txt", media_type="text/plain")],
    )
    result = await convert_to_openai_agents_messages(
        [
            message("good", cached=cached("good")),
            message("forged", cached=cached("forged", role="assistant", text="wrong role")),
            message("other", cached=cached("someone-else")),
            message("image", cached={**cached("image"), "content": [{"type": "input_image"}]}),
            file,
            *(message(f"m{i}") for i in range(101)),
        ],
        context=context,
    )
    assert texts(result[:4]) == ["cached", "Hello", "Hello", "Hello"]
    assert "file content" in texts([result[4]])[1]
    assert [len(batch) for batch in context.batches] == [100, 4]
    cached_ids = {entry_id for batch in context.batches for entry_id, _ in batch}
    assert "file" not in cached_ids and "good" not in cached_ids
    entry = dict(context.batches[0])["forged"]
    assert entry == {
        "id": "forged",
        "role": "user",
        "content": [{"type": "input_text", "text": "Hello"}],
    }


async def test_incomplete_messages_are_skipped_unless_a_message_handler_is_given() -> None:
    pending = message("p")
    pending.message.status = "pending"
    assert await convert_to_openai_agents_messages([pending]) == []
    handled = await convert_to_openai_agents_messages(
        [pending],
        on_message=MessageHandlers(
            message=lambda item: {
                "role": "user",
                "content": [{"type": "input_text", "text": item.message.status}],
            }
        ),
    )
    assert texts(handled) == ["pending"]


async def test_text_and_png_attachments_hydrate_into_text_and_image_parts() -> None:
    context = FakeContext()
    result = await convert_to_openai_agents_messages(
        [
            message(
                "m",
                text="",
                attachments=[
                    Attachment(id="image", filename="picture.png", media_type="image/png"),
                    Attachment(id="file-id", filename="notes.txt", media_type="text/plain"),
                    Attachment(id="blob", filename="data.bin", media_type="application/zip"),
                ],
            )
        ],
        context=context,
    )
    parts = result[0]["content"]
    assert parts[0] == {
        "type": "input_image",
        "detail": "auto",
        "image_url": f"data:image/png;base64,{base64.b64encode(PNG).decode()}",
    }
    assert parts[1] == {"type": "input_text", "text": "Attached file: notes.txt\nfile content"}
    assert parts[2]["text"].startswith("Attached file: data.bin (application/zip).")
    assert context.batches == []


async def test_custom_attachment_handler_receives_the_scoped_download() -> None:
    async def on_attachment(input):
        file = await input.download()
        assert file.attachment.id == input.attachment.id
        return {
            "type": "input_text",
            "text": f"Custom {input.attachment.filename}: {len(file.content)}",
        }

    result = await convert_to_openai_agents_messages(
        [message("m", attachments=[Attachment(id="image", filename="picture.png")])],
        context=FakeContext(),
        on_attachment=on_attachment,
    )
    assert texts(result) == ["Hello", f"Custom picture.png: {len(PNG)}"]


async def test_attachments_are_not_silently_discarded_without_a_scoped_downloader() -> None:
    with pytest.raises(RuntimeError, match="requires context"):
        await convert_to_openai_agents_messages(
            [
                message(
                    "m",
                    text="",
                    attachments=[
                        Attachment(id="image", filename="picture.png", media_type="image/png")
                    ],
                )
            ]
        )
