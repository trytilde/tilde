"""Dial-in host lifecycle across the real Connect boundary: wakes, controls, tools, reports."""

from __future__ import annotations

import asyncio
import json
import uuid

import pytest
from fake_gateway import FakeGateway
from support import serve_app

from tilde import (
    AgentContext,
    BundledOptions,
    StopLoop,
    ToolAnnotations,
    agent_log_processor,
    connect_agent,
)
from tilde._tools import bundled_tool, tool_spec
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


async def test_bundled_tools_are_published_run_through_execute_and_audited(dial_in):
    seen: dict[str, object] = {}

    async def run(ctx: AgentContext) -> None:
        # What an adapter does: the routed executor audits the call itself, once.
        async def read_file(input: dict, call: str) -> dict:
            async def body() -> dict:
                return {"text": f"contents of {input['path']}"}

            return await ctx._run_audited("read_file", call, input, body)

        await ctx._set_bundled_tools(
            {
                "read_file": bundled_tool(
                    tool_spec(
                        "read_file",
                        "Read a file from the workspace.",
                        {"type": "object", "properties": {"path": {"type": "string"}}},
                        output_schema={
                            "type": "object",
                            "properties": {"text": {"type": "string"}},
                        },
                        annotations=ToolAnnotations(open_world=True),
                        options=BundledOptions(
                            summary="Read a file",
                            display="summary",
                            annotations=ToolAnnotations(read_only=True, idempotent=True),
                        ),
                    ),
                    read_file,
                )
            }
        )
        seen["agent_tools"] = sorted(ctx.agent_tools)
        # Found by search elsewhere, run here: tools.execute naming it never reaches Tilde.
        seen["output"] = await ctx.tools["tools.execute"].execute(
            {"name": "read_file", "input": {"path": "a.txt"}}, tool_call_id="call-1"
        )
        with pytest.raises(ValueError, match="conflicts"):
            await ctx._set_bundled_tools({"tools.execute": ctx.tools["tools.execute"]})
        # The set changes mid-run; the next registration replaces it, and tools.execute naming
        # the removed tool goes to Tilde.
        await ctx._set_bundled_tools({})
        seen["routed"] = await ctx.tools["tools.execute"].execute(
            {"name": "read_file", "input": {"path": "b.txt"}}, tool_call_id="call-2"
        )
        ctx.stop()

    gateway, url, _host = await dial_in(run)
    gateway.tools = [
        types.ToolDefinition(
            name="tools.execute",
            provider_id="tilde",
            description="Call a tool found with tools.search.",
            input_schema_json="{}",
        )
    ]
    gateway.wakes.put_nowait(wake(url, "bundled"))
    await gateway.stopped()

    assert seen["output"] == {"text": "contents of a.txt"}
    assert seen["agent_tools"] == ["tools.execute"]
    assert seen["routed"] == {
        "ok": True,
        "input": {"name": "read_file", "input": {"path": "b.txt"}},
    }
    assert [frames[0].name for frames in gateway.tool_calls] == ["tools.execute"]
    # Registered on every invocation, again with the agent's bundled tools, and again without
    # them. SDK helpers such as stop are not bundled tools.
    first, with_bundled, without = gateway.bundled_registrations
    assert first == [] and without == []
    (read,) = with_bundled
    assert read.name == "read_file" and read.display == types.TOOL_DISPLAY_SUMMARY
    assert read.summary == "Read a file"
    assert json.loads(read.output_schema_json)["properties"]["text"] == {"type": "string"}
    # Options override what the framework declared.
    assert read.annotations.read_only and read.annotations.idempotent
    assert not read.annotations.open_world
    assert [(c.name, c.status, c.summary, c.display) for c in gateway.tool_reports] == [
        ("read_file", "running", "Read a file", types.TOOL_DISPLAY_SUMMARY),
        ("read_file", "completed", "Read a file", types.TOOL_DISPLAY_SUMMARY),
    ]
    assert json.loads(gateway.tool_reports[1].output_json) == {"text": "contents of a.txt"}


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


async def test_inference_routes_provider_clients_through_the_gateway_with_the_invocation_token(
    dial_in,
):
    import httpx

    seen: list[httpx.Request] = []
    done = asyncio.Event()

    async def run(ctx: AgentContext) -> None:
        try:
            with pytest.raises(ValueError):
                ctx.inference("not a slug")
            inference = ctx.inference("openai/prod")
            # A provider SDK sets its own key header; the auth hook replaces it with the token.
            async with inference.async_client(
                transport=httpx.MockTransport(lambda r: seen.append(r) or httpx.Response(200)),
                headers={"authorization": f"Bearer {inference.api_key}"},
            ) as client:
                await client.post(f"{inference.base_url}/chat/completions", json={})
            with inference.client(
                transport=httpx.MockTransport(lambda r: seen.append(r) or httpx.Response(200))
            ) as client:
                client.post(f"{ctx.inference('default').base_url}/responses", json={})
        finally:
            done.set()

    gateway, url, _host = await dial_in(run)
    gateway.wakes.put_nowait(wake(url, "Answer"))
    await asyncio.wait_for(done.wait(), 10)
    assert [str(r.url) for r in seen] == [
        f"{url}/inference/openai/prod/chat/completions",
        f"{url}/inference/default/responses",
    ]
    assert {r.headers["authorization"] for r in seen} == {"Bearer capability-token"}


async def test_module_level_inference_resolves_the_running_invocation_and_its_prompt_stamps(
    dial_in,
):
    import httpx

    import tilde

    # Built at import time, before any invocation exists.
    model = tilde.inference("openai/prod")
    system = tilde.define_prompt("system", template="You are {{name}}.")
    seen: list[httpx.Request] = []
    transport = httpx.MockTransport(lambda r: seen.append(r) or httpx.Response(200))
    with (
        pytest.raises(RuntimeError, match="outside a Tilde invocation"),
        model.client(transport=transport) as client,
    ):
        client.post(f"{model.base_url}/chat/completions", json={})
    done = asyncio.Event()

    async def run(ctx: AgentContext) -> None:
        try:
            async with model.async_client(transport=transport) as client:
                await client.post(f"{model.base_url}/chat/completions", json={})
                assert system.render(name="Bot") == "You are Bot."
                # Worker threads (CrewAI runs sync LLM calls on them) see the same invocation.
                await asyncio.to_thread(
                    lambda: model.prepare(httpx.Request("POST", f"{model.base_url}/responses"))
                )
                await client.post(f"{model.base_url}/responses", json={})
        finally:
            done.set()

    gateway, url, _host = await dial_in(run)
    gateway.wakes.put_nowait(wake(url, "Answer"))
    await asyncio.wait_for(done.wait(), 10)
    assert [str(r.url) for r in seen] == [
        f"{url}/inference/openai/prod/chat/completions",
        f"{url}/inference/openai/prod/responses",
    ]
    host = url.split("://", 1)[1]
    assert [r.headers["host"] for r in seen] == [host, host]
    assert {r.headers["authorization"] for r in seen} == {"Bearer capability-token"}
    assert "x-tilde-prompt" not in seen[0].headers
    assert seen[1].headers["x-tilde-prompt"] == system.stamp


async def test_skill_directory_keeps_registry_skills_per_agent_and_updates_by_version(
    dial_in, tmp_path, monkeypatch
):
    import os

    from tilde import skills as skills_module
    from tilde.agent_host.v1.agent_pb2 import InvocationState
    from tilde.runtime.v1.skills_pb2 import ReadSkillFileResponse, SkillFileInfo, SkillSummary

    monkeypatch.setenv("TILDE_SKILLS_DIR", str(tmp_path))
    monkeypatch.setattr(skills_module, "_LOCALS", {})
    logo = bytes(range(256))
    result: dict[str, object] = {}
    done = asyncio.Event()

    def research(version: str) -> SkillSummary:
        return SkillSummary(
            name="research",
            source="team",
            description="Research carefully.",
            version_id=version,
            files=[
                SkillFileInfo(path="SKILL.md"),
                SkillFileInfo(path="assets/logo.png"),
                SkillFileInfo(path="scripts/run.sh", executable=True),
            ],
        )

    shipped = SkillSummary(
        name="reply-style",
        source="agent-1",
        description="Shipped beside the code.",
        version_id="v-shipped",
        files=[SkillFileInfo(path="SKILL.md")],
        deployed=True,
    )

    async def run(ctx: AgentContext) -> None:
        try:
            if "first" in result:
                # Pushed with the wake: no list call, and only the new version is fetched.
                lists, reads = gateway.skill_lists, len(gateway.skill_reads)
                if result.get("concurrent"):
                    roots = await asyncio.gather(ctx.skills.directory(), ctx.skills.directory())
                    assert roots[0] == roots[1]
                result.update(
                    pushed=await ctx.skills.directory(),
                    pushed_lists=gateway.skill_lists - lists,
                    pushed_reads=len(gateway.skill_reads) - reads,
                )
                return
            first = await ctx.skills.directory()
            reads = len(gateway.skill_reads)
            second = await ctx.skills.directory()
            tool = ctx.skills.tools()["read_skill"]
            result.update(
                first=first,
                second=second,
                reads_after_first=reads,
                reads_after_second=len(gateway.skill_reads),
                summary=await ctx.skills.summary(),
                read=await tool.execute({"name": "research"}),
            )
        finally:
            done.set()

    gateway, url, _host = await dial_in(run)
    gateway.skills = [shipped, research("v-1")]
    gateway.skill_files = {
        ("research", "SKILL.md"): ReadSkillFileResponse(
            content="# Research", media_type="text/markdown"
        ),
        ("research", "assets/logo.png"): ReadSkillFileResponse(
            download_url=f"{url}/blobs/logo", media_type="image/png"
        ),
        ("research", "scripts/run.sh"): ReadSkillFileResponse(
            content="#!/bin/sh\necho ok\n", media_type="text/x-shellscript"
        ),
    }
    gateway.blobs["logo"] = logo
    gateway.wakes.put_nowait(wake(url, "Answer"))
    await asyncio.wait_for(done.wait(), 10)
    root = tmp_path / "tilde-skills" / "agent-1"
    assert result["first"] == result["second"] == str(root)
    assert sorted(os.listdir(root)) == [".versions.json", "research"]
    assert (root / "research" / "SKILL.md").read_text() == "# Research"
    assert (root / "research" / "assets" / "logo.png").read_bytes() == logo
    # Scripts keep their execute bit; other files get the default mode.
    assert (root / "research" / "scripts" / "run.sh").stat().st_mode & 0o777 == 0o755
    assert (root / "research" / "SKILL.md").stat().st_mode & 0o111 == 0
    # Deployed skills are never fetched; unchanged versions are not read again.
    assert result["reads_after_first"] == 3
    assert result["reads_after_second"] == 3
    assert "- reply-style: Shipped beside the code." in str(result["summary"])
    assert result["read"] == "# Research"

    done.clear()
    pushed = wake(url, "Again", command_id="cmd-2")
    pushed.state.CopyFrom(InvocationState(skills=[shipped, research("v-2")]))
    gateway.wakes.put_nowait(pushed)
    await asyncio.wait_for(done.wait(), 10)
    assert result["pushed"] == str(root)
    assert result["pushed_lists"] == 0
    assert result["pushed_reads"] == 3
    assert json.loads((root / ".versions.json").read_text()) == {"research": "v-2"}

    async def wake_with(skills: list[SkillSummary], command: str) -> None:
        done.clear()
        request = wake(url, "Again", command_id=command)
        request.state.CopyFrom(InvocationState(skills=skills))
        gateway.wakes.put_nowait(request)
        await asyncio.wait_for(done.wait(), 10)

    # A restarted agent (a fresh process) reuses the folder through .versions.json.
    monkeypatch.setattr(skills_module, "_LOCALS", {})
    await wake_with([shipped, research("v-2")], "cmd-3")
    assert result["pushed_reads"] == 0

    # Two invocations syncing at once download the new version once (twice over would be 6).
    result["concurrent"] = True
    await wake_with([shipped, research("v-3")], "cmd-4")
    assert result["pushed_reads"] == 3
    assert result["pushed"] == str(root)

    # A skill no longer given is deleted.
    result.pop("concurrent")
    await wake_with([shipped], "cmd-5")
    assert sorted(os.listdir(root)) == [".versions.json"]
    assert json.loads((root / ".versions.json").read_text()) == {}
