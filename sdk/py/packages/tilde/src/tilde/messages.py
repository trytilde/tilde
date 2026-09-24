"""Invocation-bound message history: typed conversation, objective, goal and task items."""

from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Literal

from tilde.types.v1.chat_pb2 import Goal, Message, Task

if TYPE_CHECKING:
    from tilde.context import AgentContext


@dataclass(slots=True)
class ConversationMessage:
    type: Literal["message"]
    id: str
    role: Literal["user", "assistant"]
    message: Message
    cached_agent_representation: Any | None = None


@dataclass(slots=True)
class ObjectiveMessage:
    type: Literal["objective"]
    id: str
    objective: str


@dataclass(slots=True)
class GoalMessage:
    type: Literal["goal"]
    id: str
    goal: Goal


@dataclass(slots=True)
class TaskMessage:
    type: Literal["task"]
    id: str
    task: Task


ContextMessage = ConversationMessage | ObjectiveMessage | GoalMessage | TaskMessage


@dataclass(slots=True)
class MessageHistory:
    items: list[ContextMessage]
    next_page_token: str


class MessageClient:
    """Invocation-bound facade: callers cannot select another thread or acting agent."""

    def __init__(self, ctx: AgentContext) -> None:
        self._ctx = ctx
        self._own = {
            p.id for p in ctx.participants if p.HasField("agent_id") and p.agent_id == ctx.agent_id
        }
        received = [
            m for m in ctx.messages if m.status == "complete" and m.participant_id not in self._own
        ]
        self._latest_received: Message | None = received[-1] if received else None

    async def history(
        self,
        *,
        limit: int | None = None,
        before_message_id: str | None = None,
        include_objective: bool = True,
        include_work: bool = False,
    ) -> MessageHistory:
        """Chronological page of typed context.

        ``include_objective`` adds the current run objective on the latest page unless it
        already matches the latest received message. ``include_work`` reads current goals and
        tasks and requires the invocation's work.read grant.
        """
        ctx = self._ctx
        page, goals, tasks = await asyncio.gather(
            ctx.get_messages(limit=limit, before_message_id=before_message_id),
            ctx.goals.list() if include_work else _empty(),
            ctx.tasks.list() if include_work else _empty(),
        )
        cached = {entry.message_id: entry.message_json for entry in page.cached_messages}
        items: list[ContextMessage] = []
        for message in page.messages:
            representation = None
            stored = cached.get(message.id)
            if stored:
                try:
                    representation = json.loads(stored)
                except ValueError:
                    representation = None  # Invalid cache never hides canonical history.
            items.append(
                ConversationMessage(
                    type="message",
                    id=message.id,
                    role="assistant" if message.participant_id in self._own else "user",
                    message=message,
                    cached_agent_representation=representation,
                )
            )
        if not before_message_id:
            received = [
                m
                for m in page.messages
                if m.status == "complete" and m.participant_id not in self._own
            ]
            # Small pages retain the latest received text when deciding on the objective.
            if received:
                self._latest_received = received[-1]
            latest = self._latest_received.text if self._latest_received else None
            if include_objective and ctx.objective and ctx.objective != latest:
                items.append(
                    ObjectiveMessage(
                        type="objective", id=f"objective:{ctx.run_id}", objective=ctx.objective
                    )
                )
        items.extend(GoalMessage(type="goal", id=f"goal:{goal.id}", goal=goal) for goal in goals)
        items.extend(TaskMessage(type="task", id=f"task:{task.id}", task=task) for task in tasks)
        return MessageHistory(items=items, next_page_token=page.next_page_token)


async def _empty() -> list[Any]:
    return []
