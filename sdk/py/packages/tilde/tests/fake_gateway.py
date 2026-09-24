"""An in-process stand-in for Tilde's runtime listener: chat callbacks, controls, wakes, reports."""

from __future__ import annotations

import asyncio
import json
from collections.abc import AsyncIterator
from dataclasses import dataclass, field

from connectrpc.code import Code
from connectrpc.errors import ConnectError

from tilde.agent_host.v1.agent_pb2 import InvokeRequest
from tilde.run.v1 import run_pb2
from tilde.run.v1.run_connect import RunService, RunServiceASGIApplication
from tilde.runtime.v1 import chat_pb2, controls_pb2
from tilde.runtime.v1.chat_connect import ChatService, ChatServiceASGIApplication
from tilde.runtime.v1.controls_connect import (
    InvocationControlService,
    InvocationControlServiceASGIApplication,
)
from tilde.types.v1 import chat_pb2 as types

NATIVE_SEND = types.ToolDefinition(
    name="sendMessage",
    provider_id="native",
    description="Send a plain-text message to this thread.",
    input_schema_json=json.dumps({"type": "object", "properties": {"text": {"type": "string"}}}),
    chunk_schema_json=json.dumps(
        {
            "type": "object",
            "properties": {"textDelta": {"type": "string"}},
            "required": ["textDelta"],
        }
    ),
)
SLACK_SEND = types.ToolDefinition(
    name="channel_0123456789abcdef0123456789abcdef.sendMessage",
    provider_id="slack",
    description="Post to Slack.",
    input_schema_json=json.dumps(
        {
            "type": "object",
            "properties": {"channelId": {"type": "string"}, "text": {"type": "string"}},
        }
    ),
)


@dataclass
class FakeGateway(ChatService, InvocationControlService, RunService):
    """Unimplemented RPCs raise UNIMPLEMENTED through the generated service defaults."""

    token: str = "capability-token"
    reports: list[run_pb2.ReportRequest] = field(default_factory=list)
    tool_calls: list[list[chat_pb2.InvokeToolRequest]] = field(default_factory=list)
    goals: list[types.Goal] = field(default_factory=list)
    tasks: list[types.Task] = field(default_factory=list)
    run_status: list[str] = field(default_factory=list)
    acknowledged: list[str] = field(default_factory=list)
    authorizations: set[str] = field(default_factory=set)
    commands: asyncio.Queue[controls_pb2.WatchCommandsResponse | None] = field(
        default_factory=asyncio.Queue
    )
    tools: list[types.ToolDefinition] = field(default_factory=lambda: [NATIVE_SEND, SLACK_SEND])
    renewals: int = 0
    wakes: asyncio.Queue[InvokeRequest | None] = field(default_factory=asyncio.Queue)
    heartbeats: list[bool] = field(default_factory=list)

    def _auth(self, ctx) -> None:
        authorization = ctx.request_headers.get("authorization", "")
        self.authorizations.add(authorization)
        if not authorization.startswith("Bearer "):
            raise ConnectError(Code.UNAUTHENTICATED, "missing token")

    async def stopped(
        self, invocation_id: str | None = None, wait_seconds: float = 10
    ) -> run_pb2.ReportRequest:
        for _ in range(int(wait_seconds / 0.02)):
            for report in self.reports:
                if report.WhichOneof("event") == "stopped" and invocation_id in (
                    None,
                    report.invocation_id,
                ):
                    return report
            await asyncio.sleep(0.02)
        raise AssertionError("No stopped report")

    # tilde.runtime.v1.ChatService ---------------------------------------------------------
    async def list_tools(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.ListToolsResponse(tools=self.tools)

    async def invoke_tool(self, request: AsyncIterator[chat_pb2.InvokeToolRequest], ctx):
        self._auth(ctx)
        frames = [frame async for frame in request]
        self.tool_calls.append(frames)
        name = frames[0].name
        if name == "sendMessage":
            text = "".join(json.loads(f.chunk_json)["textDelta"] for f in frames if f.chunk_json)
            message = {"id": "m-1", "threadId": "thread-1", "text": text, "status": "complete"}
            return chat_pb2.InvokeToolResponse(output_json=json.dumps(message))
        return chat_pb2.InvokeToolResponse(
            output_json=json.dumps({"ok": True, "input": json.loads(frames[0].input_json)})
        )

    async def list_goals(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.ListGoalsResponse(goals=self.goals)

    async def create_goal(self, request, ctx):
        self._auth(ctx)
        goal = types.Goal(id=request.id, objective=request.objective, status="active")
        self.goals.append(goal)
        return chat_pb2.CreateGoalResponse(goal=goal)

    async def update_goal(self, request, ctx):
        self._auth(ctx)
        for goal in self.goals:
            if goal.id == request.id:
                goal.status = request.status
                return chat_pb2.UpdateGoalResponse(goal=goal)
        raise ConnectError(Code.NOT_FOUND, "goal")

    async def list_tasks(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.ListTasksResponse(tasks=self.tasks)

    async def create_task(self, request, ctx):
        self._auth(ctx)
        task = types.Task(
            id=request.id,
            title=request.title,
            status="pending",
            dependency_ids=request.dependency_ids,
        )
        if request.HasField("goal_id"):
            task.goal_id = request.goal_id
        self.tasks.append(task)
        return chat_pb2.CreateTaskResponse(task=task)

    async def update_task(self, request, ctx):
        self._auth(ctx)
        for task in self.tasks:
            if task.id == request.id:
                task.status = request.status
                task.blocked_reason = request.blocked_reason
                return chat_pb2.UpdateTaskResponse(task=task)
        raise ConnectError(Code.NOT_FOUND, "task")

    async def set_run_status(self, request, ctx):
        self._auth(ctx)
        self.run_status.append(request.status)
        return chat_pb2.SetRunStatusResponse()

    async def renew_connect_token(self, request, ctx):
        self._auth(ctx)
        self.renewals += 1
        return chat_pb2.RenewConnectTokenResponse(token=f"renewed-{self.renewals}", expires_in=300)

    async def list_messages(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.ListMessagesResponse(
            messages=[
                types.Message(
                    id="m-0",
                    thread_id="thread-1",
                    participant_id="p-user",
                    text="Earlier",
                    status="complete",
                )
            ],
            next_page_token="",
        )

    async def cache_converted_messages(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.CacheConvertedMessagesResponse(success=True)

    async def download_attachment(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.DownloadAttachmentResponse(
            attachment=types.Attachment(
                id=request.attachment_id, filename="notes.txt", media_type="text/plain"
            ),
            content=b"file content",
        )

    async def report_tool_call(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.ReportToolCallResponse()

    async def set_typing(self, request, ctx):
        self._auth(ctx)
        return chat_pb2.SetTypingResponse()

    # tilde.runtime.v1.InvocationControlService --------------------------------------------
    async def watch_commands(self, request, ctx):
        self._auth(ctx)
        yield controls_pb2.WatchCommandsResponse(
            id="ready", kind=controls_pb2.INVOCATION_COMMAND_KIND_READY
        )
        while True:
            command = await self.commands.get()
            if command is None:
                return
            yield command

    async def acknowledge_command(self, request, ctx):
        self._auth(ctx)
        self.acknowledged.append(request.id)
        return controls_pb2.AcknowledgeCommandResponse()

    # tilde.run.v1.RunService ----------------------------------------------------------------
    async def report(self, request, ctx):
        self._auth(ctx)
        self.reports.append(request)
        if request.WhichOneof("event") == "stopped":
            # A real gateway ends the control stream once the invocation is over.
            self.commands.put_nowait(None)
        return run_pb2.ReportResponse()

    async def watch(self, request, ctx):
        self._auth(ctx)
        yield run_pb2.WatchResponse(
            registered=run_pb2.RunRegistered(
                agent_id="agent-1", deployment_id="deployment-1", instance_id=request.instance_id
            )
        )
        while True:
            wake = await self.wakes.get()
            if wake is None:
                return
            yield run_pb2.WatchResponse(wake=wake)

    async def heartbeat(self, request, ctx):
        self._auth(ctx)
        self.heartbeats.append(request.ready)
        return run_pb2.HeartbeatResponse()

    def app(self):
        apps = [
            ChatServiceASGIApplication(self),
            InvocationControlServiceASGIApplication(self),
            RunServiceASGIApplication(self),
        ]

        async def router(scope, receive, send):
            if scope["type"] == "lifespan":
                return await apps[0](scope, receive, send)
            for app in apps:
                if scope["path"].startswith(app.path + "/"):
                    return await app(scope, receive, send)
            await send({"type": "http.response.start", "status": 404, "headers": []})
            await send({"type": "http.response.body", "body": b""})

        return router
