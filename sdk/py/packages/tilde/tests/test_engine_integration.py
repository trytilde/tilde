"""Python dial-in host against the real Tilde runtime: registration, wake, streaming, work state.

Run through `task test:sdk:py:integration` (disposable Postgres and a debug engine binary).
"""

from __future__ import annotations

import asyncio
import base64
import os
import re
import secrets
import tempfile
import traceback
import uuid
from pathlib import Path

import pytest

from tilde import (
    AgentContext,
    connect_agent,
    create_management_client,
    create_tilde_chat_client,
)
from tilde.management.v1.access_pb2 import ListChannelAccessRequest, SetChannelAccessRequest
from tilde.management.v1.agents_pb2 import CreateAgentRequest
from tilde.management.v1.deployments_pb2 import RegisterDeploymentRequest
from tilde.management.v1.tilde_chat_pb2 import GetCredentialsRequest
from tilde.provider.tilde.v1 import chat_pb2 as provider_chat
from tilde.types.v1.access_pb2 import CHANNEL_ACCESS_MODE_PUBLIC
from tilde.types.v1.agent_pb2 import (
    BINARY_PERMISSION_YES,
    TARGET_SELECTION_ALL,
    Capabilities,
    TargetPermission,
)
from tilde.types.v1.chat_pb2 import ParticipantRef
from tilde.types.v1.deployment_pb2 import DEPLOYMENT_SOURCE_MANUAL, DEPLOYMENT_TARGET_GATEWAY

pytestmark = pytest.mark.integration
binary = Path(os.environ.get("ENGINE_TEST_BINARY", "target/debug/tilde"))
if not os.environ.get("TEST_DATABASE_URL") or not binary.is_file():
    pytest.skip("requires scripts/with-postgres.sh and ENGINE_TEST_BINARY", allow_module_level=True)


async def start_engine() -> tuple[asyncio.subprocess.Process, str]:
    env = {
        **os.environ,
        "DATABASE_URL": os.environ["TEST_DATABASE_URL"],
        "ENGINE_ENCRYPTION_BACKEND": "seed",
        "ENGINE_ENCRYPTION_KEY": base64.b64encode(secrets.token_bytes(32)).decode(),
        "ENGINE_KMS_KEY_ID": "",
        "RUST_LOG": "tilde=info",
        "API_PORT": "18211",
        "WEB_PORT": "18212",
        "LOGS_QUEUE_DIR": tempfile.mkdtemp(prefix="tilde-log-queue-"),
    }
    env.pop("ENGINE_PUBLIC_URL", None)
    process = await asyncio.create_subprocess_exec(
        str(binary),
        "--listen",
        "127.0.0.1:0",
        env=env,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.STDOUT,
    )
    assert process.stdout is not None
    logs = ""
    async with asyncio.timeout(30):
        while True:
            line = (await process.stdout.readline()).decode()
            if not line:
                raise AssertionError(f"Tilde exited: {logs}")
            logs += line
            match = re.search(r" address=(127\.0\.0\.1:\d+)", line)
            if match:
                break

    async def drain() -> None:
        while line := await process.stdout.readline():
            engine_logs.append(line.decode())

    drain_task = asyncio.create_task(drain())
    process.drain_task = drain_task  # type: ignore[attr-defined]
    return process, f"http://{match.group(1)}"


engine_logs: list[str] = []


async def test_python_host_serves_a_real_invocation(served):
    try:
        async with asyncio.timeout(120):
            await _exercise()
    except BaseException:
        # Teardown can outlive the report; show the cause immediately.
        traceback.print_exc()
        raise


async def _exercise():
    process, url = await start_engine()
    connected = None
    try:
        seen: dict[str, object] = {}
        errors: list[str] = []
        finished = asyncio.Event()

        async def run(ctx: AgentContext) -> None:
            try:
                seen["provider"] = ctx.tools["sendMessage"].provider_id
                await ctx.reason("private reasoning")
                goal = await ctx.goals.create(objective="Reply explicitly")
                task = await ctx.tasks.create(title="Send reply", goal_id=goal.id)
                await ctx.tasks.update(id=task.id, status="working")
                history = await ctx.message.history(include_work=True)
                seen["history"] = [item.type for item in history.items]

                async def words():
                    yield "Hello "
                    yield "from Python"

                message = await ctx.send_native_message(words())
                seen["text"] = message.text
                await ctx.tasks.update(id=task.id, status="completed")
                await ctx.goals.update(id=goal.id, status="completed")
                await ctx.set_run_status("completed")
                ctx.stop()
            except BaseException as error:
                if not ctx.cancelled:
                    errors.append(traceback.format_exc() or repr(error))
                raise
            finally:
                finished.set()

        tilde = create_management_client(url)
        agent = (
            await tilde.agents.create_agent(
                CreateAgentRequest(
                    name="Python coordinator",
                    capabilities=Capabilities(
                        tools_invoke=TargetPermission(mode=TARGET_SELECTION_ALL),
                        thread_read=BINARY_PERMISSION_YES,
                        work_read=BINARY_PERMISSION_YES,
                        work_write=BINARY_PERMISSION_YES,
                        run_update=BINARY_PERMISSION_YES,
                    ),
                )
            )
        ).agent
        deployment = await tilde.deployments.register_deployment(
            RegisterDeploymentRequest(
                agent_id=agent.id,
                source=DEPLOYMENT_SOURCE_MANUAL,
                target=DEPLOYMENT_TARGET_GATEWAY,
            )
        )
        registered = asyncio.Event()
        connected = connect_agent(
            gateway_url=url,
            deployment_token=deployment.token,
            run=run,
            on_registered=lambda _frame: registered.set(),
        )
        await asyncio.wait_for(registered.wait(), 30)
        assert connected.registration.agent_id == agent.id
        # Management provisions credentials; chat operations use the Tilde chat provider.
        credentials = await tilde.tilde_chat.get_credentials(
            GetCredentialsRequest(agent_id=agent.id)
        )
        # The Tilde channel starts private; open it so the application's identities are admitted.
        routes = (
            await tilde.access.list_channel_access(ListChannelAccessRequest(agent_id=agent.id))
        ).routes
        tilde_route = next(route for route in routes if route.provider_id == "tilde")
        await tilde.access.set_channel_access(
            SetChannelAccessRequest(
                agent_id=agent.id,
                connection_id=tilde_route.connection_id,
                mode=CHANNEL_ACCESS_MODE_PUBLIC,
            )
        )
        chat = create_tilde_chat_client(
            url, agent.id, api_key=credentials.api_key, identity="alice"
        )
        alice = (await chat.get_identity(provider_chat.GetIdentityRequest())).user
        thread = (
            await chat.create_thread(
                provider_chat.CreateThreadRequest(
                    title="Python",
                    primary_agent_id=agent.id,
                    participants=[
                        ParticipantRef(user_id=alice.id),
                        ParticipantRef(agent_id=agent.id),
                    ],
                )
            )
        ).thread
        participant = next(p for p in thread.participants if p.user_id == alice.id)
        await chat.post_message(
            provider_chat.PostMessageRequest(
                id=str(uuid.uuid4()),
                thread_id=thread.id,
                participant_id=participant.id,
                text="stream",
            )
        )
        await asyncio.wait_for(finished.wait(), 30)
        assert not errors, errors
        assert seen["provider"] == "native"
        assert seen["text"] == "Hello from Python"
        assert (
            seen["history"][0] == "message"
            and "goal" in seen["history"]
            and "task" in seen["history"]
        )
        async with asyncio.timeout(30):
            while True:
                messages = (
                    await chat.list_messages(provider_chat.ListMessagesRequest(thread_id=thread.id))
                ).messages
                reply = [
                    m for m in messages if m.text == "Hello from Python" and m.status == "complete"
                ]
                if reply:
                    break
                await asyncio.sleep(0.1)
        assert reply[0].participant_id != participant.id
    except BaseException:
        print("".join(engine_logs[-40:]))
        raise
    finally:
        if connected is not None:
            await connected.close()
        process.terminate()
        await asyncio.wait_for(process.wait(), 10)
