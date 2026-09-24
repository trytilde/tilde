"""Invocation-bound SDK context. The model cannot choose its acting agent or thread."""

from __future__ import annotations

import asyncio
import json
import logging
import uuid
from collections.abc import AsyncIterable, AsyncIterator, Awaitable, Callable
from dataclasses import dataclass
from typing import Any, Literal

from connectrpc.code import Code
from connectrpc.errors import ConnectError
from google.protobuf.json_format import ParseDict
from opentelemetry import propagate

from tilde._cancel import Cancellation, StopLoop
from tilde._tools import Tool, ToolCatalog, as_object, as_string, tool_id
from tilde._transport import http_client
from tilde.agent_host.v1.agent_pb2 import InvokeRequest
from tilde.channels import Channels
from tilde.messages import MessageClient
from tilde.run.v1 import run_pb2
from tilde.run.v1.run_connect import RunServiceClient
from tilde.runtime.v1 import agents_pb2, chat_pb2, controls_pb2
from tilde.runtime.v1.agents_connect import AgentServiceClient
from tilde.runtime.v1.cache_pb2 import CachedAgentRepresentation
from tilde.runtime.v1.chat_connect import ChatServiceClient
from tilde.runtime.v1.controls_connect import InvocationControlServiceClient
from tilde.types.v1.agent_pb2 import Agent, Capabilities
from tilde.types.v1.chat_pb2 import Attachment, Goal, Message, Participant, Run, Task, ToolCall

log = logging.getLogger("tilde")

GoalStatus = Literal["active", "completed", "failed", "canceled"]
TaskStatus = Literal["pending", "working", "blocked", "completed", "failed", "canceled"]
RunStatus = Literal["waiting", "completed", "failed", "canceled"]
ToolCallStatus = Literal["running", "completed", "failed", "aborted"]
ReportEvent = run_pb2.RunAccepted | run_pb2.RunStopped | str


@dataclass(slots=True)
class SteeringInput:
    id: str
    text: str
    message: Message | None = None


@dataclass(slots=True)
class DownloadedAttachment:
    attachment: Attachment
    content: bytes


class _Goals:
    def __init__(self, ctx: AgentContext) -> None:
        self._ctx = ctx

    async def list(self) -> list[Goal]:
        return list(
            (
                await self._ctx._session.list_goals(chat_pb2.ListGoalsRequest(), **self._ctx._rpc())
            ).goals
        )

    async def create(self, *, objective: str, id: str | None = None) -> Goal:
        request = chat_pb2.CreateGoalRequest(id=id or str(uuid.uuid4()), objective=objective)
        return (await self._ctx._session.create_goal(request, **self._ctx._rpc())).goal

    async def update(self, *, id: str, status: GoalStatus) -> Goal:
        request = chat_pb2.UpdateGoalRequest(id=id, status=status)
        return (await self._ctx._session.update_goal(request, **self._ctx._rpc())).goal


class _Tasks:
    def __init__(self, ctx: AgentContext) -> None:
        self._ctx = ctx

    async def list(self) -> list[Task]:
        return list(
            (
                await self._ctx._session.list_tasks(chat_pb2.ListTasksRequest(), **self._ctx._rpc())
            ).tasks
        )

    async def create(
        self,
        *,
        title: str,
        id: str | None = None,
        goal_id: str | None = None,
        dependency_ids: list[str] | None = None,
    ) -> Task:
        request = chat_pb2.CreateTaskRequest(
            id=id or str(uuid.uuid4()), title=title, dependency_ids=dependency_ids or []
        )
        if goal_id is not None:
            request.goal_id = goal_id
        return (await self._ctx._session.create_task(request, **self._ctx._rpc())).task

    async def update(self, *, id: str, status: TaskStatus, blocked_reason: str = "") -> Task:
        request = chat_pb2.UpdateTaskRequest(id=id, status=status, blocked_reason=blocked_reason)
        return (await self._ctx._session.update_task(request, **self._ctx._rpc())).task


class _Agents:
    """Registry operations with the current invocation token; the server checks every grant."""

    def __init__(self, ctx: AgentContext, client: AgentServiceClient) -> None:
        self._ctx = ctx
        self._client = client

    async def create(
        self,
        *,
        name: str,
        capabilities: Capabilities | None = None,
        id: str | None = None,
        concurrency_policy: int | None = None,
    ) -> Agent:
        request = agents_pb2.CreateAgentRequest(id=id or "", name=name)
        if capabilities is not None:
            request.capabilities.CopyFrom(capabilities)
        if concurrency_policy is not None:
            request.concurrency_policy = concurrency_policy
        return (await self._client.create_agent(request, **self._ctx._rpc())).agent

    async def get(self, id: str) -> Agent:
        return (
            await self._client.get_agent(agents_pb2.GetAgentRequest(id=id), **self._ctx._rpc())
        ).agent

    async def list(
        self, *, page_size: int = 0, page_token: str = ""
    ) -> agents_pb2.ListAgentsResponse:
        request = agents_pb2.ListAgentsRequest(page_size=page_size, page_token=page_token)
        return await self._client.list_agents(request, **self._ctx._rpc())

    async def update(self, request: agents_pb2.UpdateAgentRequest) -> Agent:
        return (await self._client.update_agent(request, **self._ctx._rpc())).agent

    async def delete(self, id: str) -> None:
        await self._client.delete_agent(agents_pb2.DeleteAgentRequest(id=id), **self._ctx._rpc())


class _Attachments:
    def __init__(self, ctx: AgentContext) -> None:
        self._ctx = ctx

    async def upload(
        self, *, filename: str, media_type: str, content: bytes, id: str | None = None
    ) -> Attachment:
        request = chat_pb2.UploadAttachmentRequest(
            id=id or str(uuid.uuid4()), filename=filename, media_type=media_type, content=content
        )
        return (await self._ctx._session.upload_attachment(request, **self._ctx._rpc())).attachment

    async def download(self, attachment_id: str) -> DownloadedAttachment:
        """Thread-scoped download through the invocation token; other threads' files are refused."""
        response = await self._ctx._session.download_attachment(
            chat_pb2.DownloadAttachmentRequest(attachment_id=attachment_id), **self._ctx._rpc()
        )
        return DownloadedAttachment(attachment=response.attachment, content=bytes(response.content))


class AgentContext:
    """Everything one invocation may do, bound to its connect token.

    ``tools`` contains SDK-local ``stop`` and goal/task helpers plus provider tools fetched from
    ``tilde.runtime.v1.ChatService.ListTools`` before ``run`` starts. Ordinary return values
    and ``reason()`` text never become chat messages.
    """

    def __init__(
        self,
        request: InvokeRequest,
        cancellation: Cancellation,
        fail: Callable[[ConnectError], None],
    ) -> None:
        self.invocation_id = request.invocation_id
        self.run_id = request.run_id
        self.thread_id = request.thread_id
        self.agent_id = request.agent_id
        self.agent_generation = request.agent_generation
        self.objective = request.objective
        self.callback_url = request.callback_url
        self._messages: list[Message] = list(request.messages)
        self.cached_messages: list[CachedAgentRepresentation] = list(request.cached_messages)
        self.participants: list[Participant] = (
            list(request.thread.participants) if request.HasField("thread") else []
        )
        self.cancellation = cancellation
        self._fail = fail
        self._authorization = f"Bearer {request.capability}"
        self._client = http_client()
        base = request.callback_url.rstrip("/")
        self._session = ChatServiceClient(base, http_client=self._client)
        self._controls = InvocationControlServiceClient(base, http_client=self._client)
        self._runs = RunServiceClient(base, http_client=self._client)
        self._steering: list[SteeringInput] = []
        self._accepted: dict[str, str] = {}
        self._provider_tool_names: set[str] = set()
        self._reports: asyncio.Queue[tuple[ReportEvent, asyncio.Future[None]] | None] = (
            asyncio.Queue()
        )
        self._report_worker = asyncio.create_task(self._drain_reports())
        self._stopped_reported = False
        self._suspension: asyncio.Task[None] | None = None
        self._renewal = asyncio.create_task(self._renew_tokens())
        cancellation.on_abort(self._renewal.cancel)
        self.goals = _Goals(self)
        self.tasks = _Tasks(self)
        self.agents = _Agents(self, AgentServiceClient(base, http_client=self._client))
        self.attachments = _Attachments(self)
        self.tools: ToolCatalog = self._local_tools()
        self.message = MessageClient(self)
        channel = (
            request.thread.channel
            if request.HasField("thread") and request.thread.HasField("channel")
            else None
        )
        incoming = None
        for message in request.messages:
            if (
                channel is not None
                and message.delivery.connection_id == channel.connection_id
                and not any(
                    p.id == message.participant_id
                    and p.HasField("agent_id")
                    and p.agent_id == self.agent_id
                    for p in self.participants
                )
            ):
                incoming = message
        self.channel = Channels(self.tools, channel, incoming)

    # ----- cancellation -------------------------------------------------------------------
    @property
    def cancelled(self) -> bool:
        return self.cancellation.aborted

    def check(self) -> None:
        """Raise if this invocation was stopped; call at framework checkpoints."""
        self.cancellation.check()

    def stop(self) -> None:
        """Always available. Ends the invocation without completing its run, goal or tasks."""
        error = StopLoop()
        self.cancellation.abort(error)
        raise error

    # ----- RPC plumbing ---------------------------------------------------------------------
    def _rpc(self, *, timeout_ms: int | None = None) -> dict[str, Any]:
        self.check()
        headers = {"authorization": self._authorization}
        propagate.inject(headers)
        options: dict[str, Any] = {"headers": headers}
        if timeout_ms is not None:
            options["timeout_ms"] = timeout_ms
        return options

    def trace_authorization(self) -> str:
        """Internal exporter callback: always the latest renewed token."""
        return self._authorization

    async def _renew_tokens(self) -> None:
        # Connect tokens last five minutes; renew with one minute left for the RPC.
        while True:
            await asyncio.sleep(4 * 60)
            try:
                renewed = await self._session.renew_connect_token(
                    chat_pb2.RenewConnectTokenRequest(), **self._rpc(timeout_ms=30_000)
                )
                self._authorization = f"Bearer {renewed.token}"
            except asyncio.CancelledError:
                raise
            except BaseException:
                self.cancellation.abort()
                return

    # ----- messages and thread state --------------------------------------------------------
    @property
    def messages(self) -> list[Message]:
        """Bounded history delivered with the wake plus steering messages accepted since."""
        return list(self._messages)

    async def get_messages(
        self, *, before_message_id: str | None = None, limit: int | None = None
    ) -> chat_pb2.ListMessagesResponse:
        request = chat_pb2.ListMessagesRequest(
            limit=limit or 0, before_message_id=before_message_id or ""
        )
        return await self._session.list_messages(request, **self._rpc())

    async def invoke_agent(
        self, *, agent_id: str, objective: str, idempotency_key: str | None = None
    ) -> Run:
        """Start work for an allowed agent already participating in the current thread."""
        request = chat_pb2.StartRunRequest(
            agent_id=agent_id,
            objective=objective,
            idempotency_key=idempotency_key or str(uuid.uuid4()),
        )
        return (await self._session.start_run(request, **self._rpc())).run

    async def cache_converted_messages(self, messages: list[tuple[str, Any]]) -> None:
        """Persist opaque converted ``(chat_message_id, json_value)`` pairs in the agent cache."""
        request = chat_pb2.CacheConvertedMessagesRequest(
            messages=[
                CachedAgentRepresentation(message_id=message_id, message_json=json.dumps(value))
                for message_id, value in messages
            ]
        )
        await self._session.cache_converted_messages(request, **self._rpc())

    async def hydrate_converted_messages(self, message_ids: list[str]) -> list[tuple[str, Any]]:
        response = await self._session.hydrate_converted_messages(
            chat_pb2.HydrateConvertedMessagesRequest(message_ids=message_ids), **self._rpc()
        )
        return [(m.message_id, json.loads(m.message_json)) for m in response.messages]

    async def report_tool_call(
        self,
        *,
        tool_call_id: str,
        name: str,
        status: ToolCallStatus,
        input: Any = None,
        output: Any = None,
        error: str = "",
        input_delta: str = "",
    ) -> None:
        """Audit local framework tools; provider tools invoked through Tilde are audited already."""
        call = ToolCall(
            id=tool_id(self.invocation_id, tool_call_id, name),
            name=name,
            status=status,
            input_json="" if input is None else json.dumps(input),
            output_json="" if output is None else json.dumps(output),
            error=error,
            input_delta=input_delta,
        )
        await self._session.report_tool_call(
            chat_pb2.ReportToolCallRequest(tool_call=call), **self._rpc()
        )

    async def set_typing(self, typing: bool) -> None:
        await self._session.set_typing(chat_pb2.SetTypingRequest(typing=typing), **self._rpc())

    async def set_run_status(self, status: RunStatus) -> None:
        await self._session.set_run_status(
            chat_pb2.SetRunStatusRequest(status=status), **self._rpc()
        )

    # ----- tools ----------------------------------------------------------------------------
    def _local_tools(self) -> ToolCatalog:
        def tool(
            description: str,
            properties: dict[str, Any],
            required: list[str],
            execute: Callable[[Any, str], Awaitable[Any]],
        ) -> Tool:
            return Tool(
                description=description,
                input_schema={
                    "type": "object",
                    "properties": properties,
                    "required": required,
                    "additionalProperties": False,
                },
                _execute=execute,
            )

        string = {"type": "string"}

        async def stop(_input: Any, _call: str) -> Any:
            self.stop()

        async def goals_list(_input: Any, _call: str) -> Any:
            return [_json(goal) for goal in await self.goals.list()]

        async def goals_create(input: Any, call: str) -> Any:
            goal = await self.goals.create(
                objective=as_string(as_object(input), "objective"),
                id=tool_id(self.invocation_id, call, "goal"),
            )
            return _json(goal)

        async def goals_update(input: Any, _call: str) -> Any:
            fields = as_object(input)
            goal = await self.goals.update(
                id=as_string(fields, "id"), status=as_string(fields, "status")
            )  # type: ignore[arg-type]
            return _json(goal)

        async def tasks_list(_input: Any, _call: str) -> Any:
            return [_json(task) for task in await self.tasks.list()]

        async def tasks_create(input: Any, call: str) -> Any:
            fields = as_object(input)
            dependencies = fields.get("dependencyIds")
            if dependencies is not None and (
                not isinstance(dependencies, list)
                or not all(isinstance(v, str) for v in dependencies)
            ):
                raise ValueError("Invalid dependencies")
            task = await self.tasks.create(
                title=as_string(fields, "title"),
                id=tool_id(self.invocation_id, call, "task"),
                goal_id=None if fields.get("goalId") is None else as_string(fields, "goalId"),
                dependency_ids=dependencies,
            )
            return _json(task)

        async def tasks_update(input: Any, _call: str) -> Any:
            fields = as_object(input)
            task = await self.tasks.update(
                id=as_string(fields, "id"),
                status=as_string(fields, "status"),  # type: ignore[arg-type]
                blocked_reason=""
                if fields.get("blockedReason") is None
                else as_string(fields, "blockedReason"),
            )
            return _json(task)

        return {
            "stop": tool(
                "Stop this invocation without completing its run, goal or tasks.", {}, [], stop
            ),
            "goals.list": tool("List your goals in this thread.", {}, [], goals_list),
            "goals.create": tool(
                "Create your desired outcome.", {"objective": string}, ["objective"], goals_create
            ),
            "goals.update": tool(
                "Change your goal status.",
                {"id": string, "status": {"enum": ["active", "completed", "failed", "canceled"]}},
                ["id", "status"],
                goals_update,
            ),
            "tasks.list": tool("List your tasks in this thread.", {}, [], tasks_list),
            "tasks.create": tool(
                "Create your task with optional existing dependencies.",
                {
                    "title": string,
                    "goalId": string,
                    "dependencyIds": {"type": "array", "items": string},
                },
                ["title"],
                tasks_create,
            ),
            "tasks.update": tool(
                "Change task status; blocked tasks include a reason.",
                {
                    "id": string,
                    "status": {
                        "enum": ["pending", "working", "blocked", "completed", "failed", "canceled"]
                    },
                    "blockedReason": string,
                },
                ["id", "status"],
                tasks_update,
            ),
        }

    async def refresh_tools(self) -> None:
        """Refresh server-authored descriptors before exposing local tools to the framework."""
        response = await self._session.list_tools(chat_pb2.ListToolsRequest(), **self._rpc())
        incoming: dict[str, Tool] = {}
        for definition in response.tools:
            name = definition.name
            if (
                not name
                or name in incoming
                or (name in self.tools and name not in self._provider_tool_names)
            ):
                raise ConnectError(Code.INTERNAL, "Conflicting provider tool catalog")
            input_schema = json.loads(definition.input_schema_json)
            chunk_schema = (
                json.loads(definition.chunk_schema_json) if definition.chunk_schema_json else None
            )

            def execute(input: Any, call: str, *, _name: str = name) -> Awaitable[Any]:
                return self._invoke_provider_tool(_name, input, None, call)

            stream = None
            if chunk_schema is not None:

                def stream(
                    input: Any, chunks: AsyncIterable[Any], call: str, *, _name: str = name
                ) -> Awaitable[Any]:
                    return self._invoke_provider_tool(_name, input, chunks, call)

            incoming[name] = Tool(
                description=definition.description,
                input_schema=input_schema,
                _execute=execute,
                provider_id=definition.provider_id,
                chunk_schema=chunk_schema,
                _stream=stream,
            )
        for name in self._provider_tool_names:
            self.tools.pop(name, None)
        self._provider_tool_names.clear()
        for name, wrapper in incoming.items():
            self.tools[name] = wrapper
            self._provider_tool_names.add(name)

    async def _invoke_provider_tool(
        self, name: str, input: Any, chunks: AsyncIterable[Any] | None, tool_call_id: str
    ) -> Any:
        self.check()
        call_id = tool_id(self.invocation_id, tool_call_id, name)

        async def frames() -> AsyncIterator[chat_pb2.InvokeToolRequest]:
            yield chat_pb2.InvokeToolRequest(
                name=name,
                call_id=call_id,
                sequence=0,
                input_json=_encode(input),
                finish=chunks is None,
            )
            if chunks is None:
                return
            sequence = 1
            async for chunk in chunks:
                yield chat_pb2.InvokeToolRequest(
                    name=name, call_id=call_id, sequence=sequence, chunk_json=_encode(chunk)
                )
                sequence += 1
            yield chat_pb2.InvokeToolRequest(
                name=name, call_id=call_id, sequence=sequence, finish=True
            )

        response = await self._session.invoke_tool(frames(), **self._rpc())
        return json.loads(response.output_json)

    async def send_native_message(
        self,
        content: str | AsyncIterable[str],
        *,
        tool_call_id: str | None = None,
        addressed_participant_ids: list[str] | None = None,
        in_reply_to_message_id: str | None = None,
        attachment_ids: list[str] | None = None,
    ) -> Message:
        """Native-thread convenience only; other providers expose their own sendMessage tool."""
        tool = self.tools.get("sendMessage")
        if tool is None or tool.provider_id != "native" or not tool.can_stream:
            raise ConnectError(Code.FAILED_PRECONDITION, "Native message tool is unavailable")

        async def chunks() -> AsyncIterator[dict[str, str]]:
            source: AsyncIterable[str]
            if isinstance(content, str):

                async def single() -> AsyncIterator[str]:
                    yield content

                source = single()
            else:
                source = content
            async for chunk in source:
                if not isinstance(chunk, str):
                    raise ValueError("Native message chunks must be strings")
                for offset in range(0, len(chunk), 4096):
                    yield {"textDelta": chunk[offset : offset + 4096]}

        input: dict[str, Any] = {"text": ""}
        if addressed_participant_ids is not None:
            input["addressedParticipantIds"] = addressed_participant_ids
        if in_reply_to_message_id is not None:
            input["inReplyToMessageId"] = in_reply_to_message_id
        if attachment_ids is not None:
            input["attachmentIds"] = attachment_ids
        output = await tool.stream(input, chunks(), tool_call_id=tool_call_id or str(uuid.uuid4()))
        return ParseDict(output, Message(), ignore_unknown_fields=True)

    # ----- run reports ----------------------------------------------------------------------
    async def reason(self, text: str) -> None:
        """Stream reasoning into execution activity, never into the thread transcript."""
        self.check()
        await self.report(text)

    def report(self, event: ReportEvent) -> Awaitable[None]:
        """Internal host hook: one ordered RunService.Report; ``stopped`` is sent at most once.

        Reports outlive the invocation signal (the final ``stopped`` is sent after it).
        Acceptance and the end of the run decide what the gateway records, so they retry a
        few times; a lost reasoning delta only costs a UI update.
        """
        future: asyncio.Future[None] = asyncio.get_running_loop().create_future()
        if isinstance(event, run_pb2.RunStopped):
            if self._stopped_reported:
                future.set_result(None)
                return future
            self._stopped_reported = True
        self._reports.put_nowait((event, future))
        return future

    async def _drain_reports(self) -> None:
        while True:
            item = await self._reports.get()
            if item is None:
                return
            event, future = item
            try:
                await self._send_report(event)
            finally:
                if not future.done():
                    future.set_result(None)

    async def _send_report(self, event: ReportEvent) -> None:
        request = run_pb2.ReportRequest(invocation_id=self.invocation_id)
        if isinstance(event, str):
            request.reasoning_delta = event
            attempts = 1
        elif isinstance(event, run_pb2.RunAccepted):
            request.accepted.CopyFrom(event)
            attempts = 4
        else:
            request.stopped.CopyFrom(event)
            attempts = 4
        for attempt in range(1, attempts + 1):
            try:
                headers = {"authorization": self._authorization}
                propagate.inject(headers)
                await self._runs.report(request, headers=headers, timeout_ms=10_000)
                return
            except ConnectError as error:
                # The gateway answered and refused: retrying cannot change that.
                permanent = error.code not in (
                    Code.UNAVAILABLE,
                    Code.DEADLINE_EXCEEDED,
                    Code.UNKNOWN,
                    Code.INTERNAL,
                )
                if permanent or attempt >= attempts:
                    log.warning(
                        "run report (%s) for invocation %s failed: %s",
                        request.WhichOneof("event"),
                        self.invocation_id,
                        error.message,
                    )
                    return
                await asyncio.sleep(0.25 * attempt)

    async def close_reports(self) -> None:
        """Internal host hook: flush ordered reports after the final ``stopped``."""
        self._reports.put_nowait(None)
        await self._report_worker

    # ----- steering -------------------------------------------------------------------------
    def take_inputs(self) -> list[SteeringInput]:
        """Consume newly steered input at the agent framework's next safe checkpoint."""
        inputs, self._steering = self._steering, []
        return inputs

    def pending_input_ids(self) -> list[str]:
        """Inputs not yet handed to the reasoning loop remain pending."""
        return [input.id for input in self._steering]

    def accept_input(self, input: SteeringInput) -> None:
        """Internal server hook; duplicate IDs replay acknowledgment, other content is rejected."""
        self.check()
        prior = self._accepted.get(input.id)
        if prior is not None:
            if prior != input.text:
                raise ConnectError(Code.ALREADY_EXISTS, "Input ID conflict")
            return
        if len(self._accepted) >= 4096:
            raise ConnectError(Code.RESOURCE_EXHAUSTED, "Too many steering inputs")
        if input.message is not None:
            if input.message.id != input.id or input.message.thread_id != self.thread_id:
                raise ConnectError(Code.INVALID_ARGUMENT, "Steering message is outside this input")
            self._messages = [m for m in self._messages if m.id != input.message.id] + [
                input.message
            ]
            self._messages = self._messages[-100:]
        self._accepted[input.id] = input.text
        self._steering.append(input)

    # ----- controls -------------------------------------------------------------------------
    async def settle_suspension(self) -> None:
        if self._suspension is not None:
            await self._suspension

    async def consume_controls(
        self,
        ready: Callable[[], None],
        checkpoint: Callable[[AgentContext], Awaitable[None]] | None,
    ) -> None:
        """The running request subscribes outward; control never needs host affinity."""
        loop = asyncio.get_running_loop()
        connected = loop.time()
        while not self.cancellation.aborted:
            try:
                stream = self._controls.watch_commands(
                    controls_pb2.WatchCommandsRequest(), **self._rpc()
                )
                try:
                    async for command in stream:
                        if await self._handle_command(command, ready, checkpoint):
                            return
                finally:
                    # Close the HTTP/2 stream now; deferred generator cleanup would leak it.
                    await stream.aclose()
                # The server closes a healthy stream at its token's expiry. Reconnect
                # with the renewed token instead of treating the stream's age as an outage.
                connected = loop.time()
            except ConnectError as error:
                if self.cancellation.aborted:
                    return
                if error.code in (
                    Code.UNAUTHENTICATED,
                    Code.PERMISSION_DENIED,
                    Code.NOT_FOUND,
                    Code.INVALID_ARGUMENT,
                    Code.ALREADY_EXISTS,
                    Code.RESOURCE_EXHAUSTED,
                ):
                    raise
            except (StopLoop, asyncio.CancelledError):
                return
            except Exception:
                if self.cancellation.aborted:
                    return
            if loop.time() - connected >= 15:
                raise ConnectError(Code.UNAVAILABLE, "Invocation control connection lost")
            waiter = asyncio.ensure_future(self.cancellation.wait())
            try:
                await asyncio.wait({waiter}, timeout=0.25)
            finally:
                waiter.cancel()

    async def _handle_command(
        self,
        command: controls_pb2.WatchCommandsResponse,
        ready: Callable[[], None],
        checkpoint: Callable[[AgentContext], Awaitable[None]] | None,
    ) -> bool:
        """Apply one control; returns True when the invocation must end (stop or suspend)."""
        kind = command.kind
        if kind == controls_pb2.INVOCATION_COMMAND_KIND_READY:
            ready()
            return False
        if kind == controls_pb2.INVOCATION_COMMAND_KIND_STEER:
            self.accept_input(
                SteeringInput(
                    id=command.input_id,
                    text=command.text,
                    message=command.message if command.HasField("message") else None,
                )
            )
            await self._controls.acknowledge_command(
                controls_pb2.AcknowledgeCommandRequest(id=command.id), **self._rpc()
            )
            return False
        if kind not in (
            controls_pb2.INVOCATION_COMMAND_KIND_STOP,
            controls_pb2.INVOCATION_COMMAND_KIND_SUSPEND,
        ):
            raise ConnectError(Code.INVALID_ARGUMENT, "Unknown invocation control")
        suspend = kind == controls_pb2.INVOCATION_COMMAND_KIND_SUSPEND

        async def complete() -> None:
            if suspend and checkpoint is not None:
                try:
                    await checkpoint(self)
                except BaseException:
                    try:
                        await self.set_run_status("failed")
                    finally:
                        self._fail(ConnectError(Code.INTERNAL, "Suspension checkpoint failed"))
                    raise
            await self._controls.acknowledge_command(
                controls_pb2.AcknowledgeCommandRequest(id=command.id), **self._rpc()
            )

        completion = asyncio.create_task(complete())
        if suspend:
            self._suspension = completion
        try:
            await completion
        finally:
            self.cancellation.abort(StopLoop())
        return True


def _encode(value: Any) -> str:
    try:
        return json.dumps(value)
    except (TypeError, ValueError) as error:
        raise ValueError("Tool inputs must be JSON values") from error


def _json(message: Any) -> Any:
    from google.protobuf.json_format import MessageToDict

    return MessageToDict(message)
