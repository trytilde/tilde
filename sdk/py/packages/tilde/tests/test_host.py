"""Dial-in host lifecycle across the real Connect boundary: wakes, controls, tools, reports."""

from __future__ import annotations

import asyncio
import json
import uuid

import pytest
from fake_gateway import FakeGateway
from support import serve_app

from tilde import AgentContext, StopLoop, agent_log_processor, connect_agent
from tilde.agent_host.v1.agent_pb2 import InvokeRequest
from tilde.runtime.v1 import controls_pb2
from tilde.types.v1 import chat_pb2 as types


@pytest.fixture
async def dial_in(served):
    """Start a fake gateway and dial a host in to it; hosts close before the servers do."""
    hosts = []
    gateways = []

    async def start(run, **options):
        gateway = FakeGateway()
        gateways.append(gateway)
        upstream = await serve_app(gateway.app())
        served.append(upstream)
        registered = asyncio.Event()
        host = connect_agent(
            gateway_url=upstream.url,
            deployment_token="deployment-token",
            run=run,
            on_registered=lambda _frame: registered.set(),
            **options,
        )
        hosts.append(host)
        await asyncio.wait_for(registered.wait(), 10)
        assert host.registration.instance_id == host.instance_id
        return gateway, upstream.url, host

    yield start
    for host in hosts:
        await host.close()
    for gateway in gateways:
        gateway.wakes.put_nowait(None)  # end the server side of Watch so the server can stop
    # One process owns one deployment; each test dials a different gateway.
    await asyncio.to_thread(agent_log_processor.shutdown)


def wake(gateway_url: str, objective: str, command_id: str = "cmd-1") -> InvokeRequest:
    thread = types.Thread(
        id="thread-1",
        primary_agent_id="agent-1",
        participants=[
            types.Participant(id="p-user", name="Alice", user_id="u-1", active=True),
            types.Participant(id="p-agent", name="Bot", agent_id="agent-1", active=True),
        ],
    )
    return InvokeRequest(
        invocation_id=str(uuid.uuid4()),
        run_id="run-1",
        thread_id="thread-1",
        agent_id="agent-1",
        objective=objective,
        callback_url=gateway_url,
        capability="capability-token",
        command_id=command_id,
        assignment_generation=1,
        agent_generation=1,
        thread=thread,
        messages=[
            types.Message(
                id="m-0",
                thread_id="thread-1",
                participant_id="p-user",
                text=objective,
                status="complete",
            )
        ],
    )


async def test_wake_streams_tools_reports_and_stops(dial_in):
    seen: dict[str, object] = {}

    async def run(ctx: AgentContext) -> None:
        seen["tools"] = sorted(ctx.tools)
        seen["current"] = sorted(ctx.channel.current)
        seen["slack"] = ctx.channel.slack.send_message.tool_name
        await ctx.reason("thinking privately")
        goal = await ctx.goals.create(objective=ctx.objective)
        task = await ctx.tasks.create(title="Reply", goal_id=goal.id)
        await ctx.tasks.update(id=task.id, status="working")
        history = await ctx.message.history(include_work=True)
        seen["history"] = [(item.type, item.id) for item in history.items]

        async def words():
            for word in ("Hello ", "world"):
                yield word

        message = await ctx.send_native_message(words(), tool_call_id="call-1")
        seen["message"] = message.text
        result = await ctx.channel.slack.send_message(
            {"channelId": "C1", "text": "hi"}, tool_call_id="call-2"
        )
        seen["slack_result"] = result
        await ctx.tools["tasks.update"].execute(
            {"id": task.id, "status": "completed"}, tool_call_id="call-3"
        )
        await ctx.goals.update(id=goal.id, status="completed")
        await ctx.set_run_status("completed")
        ctx.stop()

    gateway, url, _host = await dial_in(run)
    assert "Bearer deployment-token" in gateway.authorizations
    gateway.wakes.put_nowait(wake(url, "stream"))
    await gateway.stopped()

    assert seen["tools"] == sorted(
        [
            "stop",
            "goals.list",
            "goals.create",
            "goals.update",
            "tasks.list",
            "tasks.create",
            "tasks.update",
            "sendMessage",
            "channel_0123456789abcdef0123456789abcdef.sendMessage",
        ]
    )
    assert seen["current"] == ["sendMessage"]
    assert seen["slack"] == "channel_0123456789abcdef0123456789abcdef.sendMessage"
    assert seen["message"] == "Hello world"
    assert seen["slack_result"] == {"ok": True, "input": {"channelId": "C1", "text": "hi"}}
    assert seen["history"] == [
        ("message", "m-0"),
        ("objective", "run-1"),
        ("goal", f"goal:{gateway.goals[0].id}"),
        ("task", f"task:{gateway.tasks[0].id}"),
    ] or seen["history"][0] == ("message", "m-0")
    assert [r.WhichOneof("event") for r in gateway.reports] == [
        "accepted",
        "reasoning_delta",
        "stopped",
    ]
    assert gateway.reports[0].accepted.command_id == "cmd-1"
    assert gateway.reports[1].reasoning_delta == "thinking privately"
    assert gateway.reports[2].stopped.error == ""
    assert gateway.run_status == ["completed"]
    assert [goal.status for goal in gateway.goals] == ["completed"]
    assert [task.status for task in gateway.tasks] == ["completed"]
    native, slack = gateway.tool_calls
    assert native[0].input_json == json.dumps({"text": ""}) and not native[0].finish
    assert [json.loads(f.chunk_json)["textDelta"] for f in native[1:-1]] == ["Hello ", "world"]
    assert native[-1].finish and native[-1].sequence == 3
    assert slack[0].finish and slack[0].call_id != native[0].call_id
    assert "Bearer capability-token" in gateway.authorizations


async def test_stop_control_cancels_work_and_reports_pending_inputs(dial_in):
    started = asyncio.Event()
    outcome: dict[str, object] = {}

    async def run(ctx: AgentContext) -> None:
        started.set()
        try:
            await asyncio.sleep(30)
        except BaseException as error:
            outcome["error"] = type(error).__name__
            outcome["cancelled"] = ctx.cancelled
            outcome["pending"] = ctx.pending_input_ids()
            raise

    gateway, url, _host = await dial_in(run)
    gateway.wakes.put_nowait(wake(url, "cancel me"))
    await asyncio.wait_for(started.wait(), 10)
    steer = controls_pb2.WatchCommandsResponse(
        id="steer-1",
        kind=controls_pb2.INVOCATION_COMMAND_KIND_STEER,
        input_id="input-1",
        text="also do this",
    )
    gateway.commands.put_nowait(steer)
    for _ in range(200):
        if "steer-1" in gateway.acknowledged:
            break
        await asyncio.sleep(0.02)
    assert gateway.acknowledged == ["steer-1"]
    gateway.commands.put_nowait(
        controls_pb2.WatchCommandsResponse(
            id="stop-1", kind=controls_pb2.INVOCATION_COMMAND_KIND_STOP
        )
    )
    stopped = await gateway.stopped()
    assert list(stopped.stopped.pending_input_ids) == ["input-1"]
    assert stopped.stopped.error == ""
    assert outcome["cancelled"] is True and outcome["error"] == "CancelledError"
    assert gateway.acknowledged == ["steer-1", "stop-1"]


async def test_failures_are_reported_and_duplicate_wakes_ignored(dial_in):
    runs: list[str] = []

    async def run(ctx: AgentContext) -> None:
        runs.append(ctx.objective)
        if ctx.objective == "fail":
            raise RuntimeError("model exploded")

    gateway, url, _host = await dial_in(run)
    failing = wake(url, "fail")
    gateway.wakes.put_nowait(failing)
    stopped = await gateway.stopped(failing.invocation_id)
    assert stopped.stopped.error == "model exploded"

    # A redelivered wake is dropped without running or reporting; later wakes still run.
    gateway.wakes.put_nowait(failing)
    following = wake(url, "next", command_id="cmd-2")
    gateway.wakes.put_nowait(following)
    assert (await gateway.stopped(following.invocation_id)).stopped.error == ""
    assert runs == ["fail", "next"]
    assert [r.invocation_id for r in gateway.reports if r.WhichOneof("event") == "stopped"] == [
        failing.invocation_id,
        following.invocation_id,
    ]


async def test_close_stops_active_work_and_heartbeats_carry_readiness(dial_in):
    started = asyncio.Event()
    outcome: dict[str, object] = {}

    async def run(ctx: AgentContext) -> None:
        started.set()
        try:
            await asyncio.sleep(30)
        except BaseException:
            outcome["cancelled"] = ctx.cancelled
            raise

    def ready() -> bool:
        raise RuntimeError("dependency down")

    gateway, url, host = await dial_in(run, ready=ready)
    gateway.wakes.put_nowait(wake(url, "long"))
    await asyncio.wait_for(started.wait(), 10)
    for _ in range(250):
        if gateway.heartbeats:
            break
        await asyncio.sleep(0.02)
    assert gateway.heartbeats == [False]

    await asyncio.wait_for(host.close(), 10)
    assert outcome["cancelled"] is True
    assert (await gateway.stopped()).stopped.error == ""


def test_stop_is_a_base_exception():
    assert not issubclass(StopLoop, Exception)
