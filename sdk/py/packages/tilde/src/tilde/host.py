"""Agent hosts: dial-in hosts (RunService.Watch) and cloud invoke handlers.

Tilde never calls an agent over HTTP; hosts dial out and receive wakes as stream frames.
"""

from __future__ import annotations

import asyncio
import logging
import os
import signal as os_signal
import uuid
from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field
from typing import Any, Literal, cast

from connectrpc.code import Code
from connectrpc.errors import ConnectError
from google.protobuf.json_format import ParseDict
from opentelemetry import context as otel_context

from tilde._cancel import Cancellation, InvocationCancelled, StopLoop
from tilde._invocation import current
from tilde._transport import http_client
from tilde.agent_host.v1.agent_pb2 import InvokeRequest
from tilde.context import AgentContext
from tilde.logging import (
    DeploymentLogging,
    agent_log_processor,
    initialize_logging,
    invocation_logging,
)
from tilde.run.v1 import run_pb2
from tilde.run.v1.run_connect import RunServiceClient
from tilde.tracing import initialize_tracing, invocation_tracing

log = logging.getLogger("tilde")

RunFn = Callable[[AgentContext], Awaitable[Any]]
CheckpointFn = Callable[[AgentContext], Awaitable[None]]
ReadyFn = Callable[[], bool | Awaitable[bool]]


@dataclass(slots=True)
class InvocationOptions:
    """What to run and how to checkpoint it, shared by every host shape."""

    run: RunFn
    checkpoint: CheckpointFn | None = None
    tracing: Literal["existing"] | None = None
    logging: Literal["existing"] | None = None
    deployment_logging: DeploymentLogging | None = None


@dataclass(slots=True)
class _VirtualThread:
    invocation_id: str
    generation: int
    cancellation: Cancellation
    completion: asyncio.Task[None]


@dataclass(slots=True)
class HostState:
    """Per-process state: a virtual conversation thread owns one reasoning loop at a time."""

    finished: dict[str, None] = field(default_factory=dict)
    virtual_threads: dict[str, _VirtualThread] = field(default_factory=dict)


@dataclass(slots=True)
class Wake:
    """How a wake reached this host; ``cancellation`` ends the invocation when the host closes."""

    cancellation: Cancellation | None = None


async def run_invocation(
    request: InvokeRequest, options: InvocationOptions, host: HostState, wake: Wake | None = None
) -> None:
    """Execute one wake to completion.

    Returns when the invocation ends (normal return, stop or suspension) and raises a
    ConnectError when it could not start or failed. Every lifecycle event is reported through
    tilde.run.v1.RunService.Report with the invocation capability. Authentication of the wake
    itself (deployment token on Watch, cloud IAM) is the caller's concern.
    """
    wake = wake or Wake()
    if request.invocation_id in host.finished:
        raise ConnectError(Code.ALREADY_EXISTS, "Invocation already accepted")
    thread_key = f"{request.agent_id}:{request.thread_id}"
    while True:
        previous = host.virtual_threads.get(thread_key)
        if previous is None:
            break
        if previous.generation >= request.assignment_generation:
            raise ConnectError(Code.ALREADY_EXISTS, "Conversation already has an active invocation")
        previous.cancellation.abort(StopLoop())
        await asyncio.wait({previous.completion})
        if wake.cancellation is not None:
            wake.cancellation.check()
        if host.virtual_threads.get(thread_key) is previous:
            del host.virtual_threads[thread_key]
        # Another wake may have acquired the thread while we awaited.

    loop = asyncio.get_running_loop()
    cancellation = Cancellation()
    done: asyncio.Future[None] = loop.create_future()

    def settle(error: ConnectError | None = None) -> None:
        if done.done():
            return
        if error is None:
            done.set_result(None)
        else:
            done.set_exception(error)

    if wake.cancellation is not None:
        outer = wake.cancellation
        outer.on_abort(lambda: cancellation.abort(outer.reason))
    context = AgentContext(request, cancellation, settle)
    ready: asyncio.Future[None] = loop.create_future()

    def resolve_ready() -> None:
        if not ready.done():
            ready.set_result(None)

    def reject_ready(error: BaseException) -> None:
        if not ready.done():
            ready.set_exception(error)

    def reject_aborted() -> None:
        reason = cancellation.reason
        reject_ready(
            reason
            if isinstance(reason, ConnectError)
            else ConnectError(Code.CANCELED, "Invocation is no longer active")
        )

    cancellation.on_abort(reject_aborted)

    async def controls() -> None:
        try:
            await context.consume_controls(resolve_ready, options.checkpoint)
        except (StopLoop, asyncio.CancelledError):
            return
        except BaseException as error:  # noqa: BLE001 - surfaced through ready/done
            connect_error = (
                error
                if isinstance(error, ConnectError)
                else ConnectError(Code.INTERNAL, str(error))
            )
            reject_ready(connect_error)
            settle(connect_error)
            cancellation.abort(connect_error)

    controls_task = asyncio.create_task(controls())
    # The wake itself carries the trace context.
    trace_headers: dict[str, str] = {}
    if request.traceparent:
        trace_headers["traceparent"] = request.traceparent
        if request.tracestate:
            trace_headers["tracestate"] = request.tracestate
    try:
        telemetry = invocation_tracing(request, trace_headers, context.trace_authorization)
        logs = invocation_logging(request, telemetry.context, context.trace_authorization)
    except Exception as error:  # noqa: BLE001 - telemetry setup must not leak a bare exception
        cancellation.abort()
        controls_task.cancel()
        await asyncio.gather(controls_task, return_exceptions=True)
        await context.close_reports()
        raise ConnectError(Code.INTERNAL, "Telemetry setup failed") from error
    trace_failed = False
    failure = ""

    async def execution() -> None:
        nonlocal trace_failed, failure
        # This task's context only: module-level tilde.inference(...) resolves this invocation.
        current.set(context)
        try:
            await asyncio.shield(ready)
            await context.refresh_tools()
            context.check()
            await options.run(context)
            await context.settle_suspension()
            settle()
        except BaseException as error:  # noqa: BLE001 - every outcome is reported, never raised
            aborted = (
                isinstance(error, StopLoop | InvocationCancelled | asyncio.CancelledError)
                or cancellation.aborted
            )
            trace_failed = not aborted
            if trace_failed:
                failure = str(error) or type(error).__name__
                log.warning("invocation %s failed", request.invocation_id, exc_info=error)
            settle(None if aborted else ConnectError(Code.INTERNAL, "Agent execution failed"))

    token = otel_context.attach(logs.context)
    try:
        execution_task = asyncio.create_task(execution())
    finally:
        otel_context.detach(token)
    cancellation.on_abort(execution_task.cancel)
    host.virtual_threads[thread_key] = _VirtualThread(
        invocation_id=request.invocation_id,
        generation=request.assignment_generation,
        cancellation=cancellation,
        completion=execution_task,
    )
    try:
        await asyncio.shield(ready)
        if request.command_id:
            await context.report(run_pb2.RunAccepted(command_id=request.command_id))
        await asyncio.shield(done)
    finally:
        cancellation.abort()
        host.finished[request.invocation_id] = None
        if len(host.finished) > 4096:
            del host.finished[next(iter(host.finished))]

        def release(_task: asyncio.Task[None]) -> None:
            current = host.virtual_threads.get(thread_key)
            if current is not None and current.invocation_id == request.invocation_id:
                del host.virtual_threads[thread_key]

        execution_task.add_done_callback(release)
        controls_task.cancel()
        await asyncio.gather(controls_task, return_exceptions=True)
        # A failed run says so, so the gateway records a failed invocation instead of a stop.
        await context.report(
            run_pb2.RunStopped(pending_input_ids=context.pending_input_ids(), error=failure)
        )
        await context.close_reports()
        # Keep the host request alive until final telemetry is flushed (including serverless hosts).
        await asyncio.gather(logs.end(), telemetry.end(trace_failed), return_exceptions=True)


@dataclass(slots=True)
class ConnectedAgent:
    """A long-running host that dialed in; ``close()`` stops watching, heartbeating and work."""

    instance_id: str
    _closed: Cancellation
    _watching: asyncio.Task[None]
    _heartbeat: asyncio.Task[None]
    _active: set[asyncio.Task[None]]
    registration: run_pb2.RunRegistered | None = None

    async def close(self) -> None:
        self._closed.abort(StopLoop())
        self._heartbeat.cancel()
        await asyncio.gather(self._heartbeat, self._watching, return_exceptions=True)
        await asyncio.gather(*self._active, return_exceptions=True)
        await asyncio.to_thread(agent_log_processor.force_flush)

    async def wait(self) -> None:
        """Block until ``close()`` is called; pair with signal handlers in a standalone host."""
        await self._closed.wait()
        await self._watching


def discovering() -> bool:
    """``tilde deploy`` imports the entry module with ``TILDE_DISCOVERY=1``: hosts do nothing."""
    return os.environ.get("TILDE_DISCOVERY") == "1"


class _DiscoveryHost:
    instance_id = ""
    registration = None

    async def close(self) -> None:
        return None

    async def wait(self) -> None:
        return None


def connect_agent(
    *,
    run: RunFn,
    gateway_url: str | None = None,
    deployment_token: str | None = None,
    checkpoint: CheckpointFn | None = None,
    ready: ReadyFn | None = None,
    instance_id: str | None = None,
    on_registered: Callable[[run_pb2.RunRegistered], None] | None = None,
    tracing: Literal["existing"] | None = None,
    logging: Literal["existing"] | None = None,
) -> ConnectedAgent:
    """Open RunService.Watch with the deployment token and run every wake frame concurrently.

    ``gateway_url`` and ``deployment_token`` default to ``TILDE_GATEWAY_URL`` and
    ``TILDE_DEPLOYMENT_TOKEN``. Heartbeats every 3 seconds, carrying the result of ``ready``
    (not ready when it raises), and reconnects after 1 second whenever the stream ends, until
    ``close()``. The host exposes no inbound endpoint. Requires a running event loop.
    Under ``TILDE_DISCOVERY=1`` it dials nothing and returns a host whose methods are no-ops.
    """
    if discovering():
        return cast(ConnectedAgent, _DiscoveryHost())
    gateway_url = gateway_url or os.environ.get("TILDE_GATEWAY_URL")
    deployment_token = deployment_token or os.environ.get("TILDE_DEPLOYMENT_TOKEN")
    if not gateway_url or not deployment_token:
        raise ValueError(
            "connect_agent needs gateway_url and deployment_token "
            "(or TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN)"
        )
    initialize_tracing(tracing == "existing")
    try:
        initialize_logging(logging == "existing", DeploymentLogging(gateway_url, deployment_token))
    except RuntimeError:
        # Startup/background logs belong to one deployment per process: the first host keeps
        # them. Invocation logs are routed per invocation and are unaffected.
        initialize_logging(logging == "existing", None)
        log.warning("deployment logs stay with the first connected deployment in this process")
    options = InvocationOptions(run=run, checkpoint=checkpoint, tracing=tracing, logging=logging)
    host = HostState()
    instance = instance_id or str(uuid.uuid4())
    client = RunServiceClient(gateway_url.rstrip("/"), http_client=http_client())
    headers = {"authorization": f"Bearer {deployment_token}"}
    closed = Cancellation()
    active: set[asyncio.Task[None]] = set()
    connected: ConnectedAgent | None = None

    async def is_ready() -> bool:
        if ready is None:
            return True
        try:
            result = ready()
            if asyncio.iscoroutine(result):
                result = await result
            return result is True
        except Exception:  # noqa: BLE001 - a throwing check reports not ready
            return False

    async def send_heartbeat() -> None:
        try:
            await client.heartbeat(
                run_pb2.HeartbeatRequest(instance_id=instance, ready=await is_ready()),
                headers=headers,
                timeout_ms=5_000,
            )
        except ConnectError as error:
            if not closed.aborted:
                log.warning("heartbeat for instance %s failed: %s", instance, error.message)

    async def heartbeat() -> None:
        while not closed.aborted:
            await asyncio.sleep(3)
            await send_heartbeat()

    def _dispatch_frame(frame: run_pb2.WatchResponse) -> None:
        kind = frame.WhichOneof("frame")
        if kind == "registered":
            assert connected is not None
            connected.registration = frame.registered
            if on_registered is not None:
                on_registered(frame.registered)
        elif kind == "wake":
            request = frame.wake

            async def invoke(request: InvokeRequest = request) -> None:
                try:
                    await run_invocation(request, options, host, Wake(cancellation=closed))
                except ConnectError as error:
                    log.warning(
                        "invocation %s ended with an error: %s",
                        request.invocation_id,
                        error.message,
                    )

            task = asyncio.create_task(invoke())
            active.add(task)
            task.add_done_callback(active.discard)

    async def watch() -> None:
        while not closed.aborted:
            try:
                stream = client.watch(
                    run_pb2.WatchRequest(instance_id=instance),
                    headers=headers,
                )
                try:
                    async for frame in stream:
                        if frame.WhichOneof("frame") == "registered":
                            # Routing needs a ready heartbeat; do not wait for the first interval.
                            await send_heartbeat()
                        _dispatch_frame(frame)
                finally:
                    await stream.aclose()
            except asyncio.CancelledError:
                raise
            except ConnectError as error:
                if closed.aborted:
                    break
                log.warning("watch for instance %s failed: %s", instance, error.message)
            except Exception as error:  # noqa: BLE001 - reconnect on any transport failure
                if closed.aborted:
                    break
                log.warning("watch for instance %s failed: %s", instance, error)
            if closed.aborted:
                break
            waiter = asyncio.ensure_future(closed.wait())
            try:
                await asyncio.wait({waiter}, timeout=1)
            finally:
                waiter.cancel()

    watching = asyncio.create_task(watch())
    closed.on_abort(watching.cancel)
    connected = ConnectedAgent(
        instance_id=instance,
        _closed=closed,
        _watching=watching,
        _heartbeat=asyncio.create_task(heartbeat()),
        _active=active,
    )
    return connected


def run_connected_agent(**options: Any) -> None:
    """Blocking standalone host: ``connect_agent(**options)`` until SIGINT/SIGTERM, then close."""
    if discovering():
        return

    async def main() -> None:
        connected = connect_agent(**options)
        stop = asyncio.Event()
        loop = asyncio.get_running_loop()
        for name in (os_signal.SIGINT, os_signal.SIGTERM):
            loop.add_signal_handler(name, stop.set)
        await stop.wait()
        await connected.close()

    asyncio.run(main())


def create_lambda_handler(
    *,
    run: RunFn,
    checkpoint: CheckpointFn | None = None,
    tracing: Literal["existing"] | None = None,
    logging: Literal["existing"] | None = None,
    deployment_logging: DeploymentLogging | None = None,
) -> Callable[[Any, Any], None]:
    """AWS Lambda host: the event is the JSON-encoded InvokeRequest from the cloud invoke API.

    Runs the invocation to completion and reports through RunService; failures are reported
    there and logged rather than raised, so the platform does not retry an invocation that
    already ended.
    """
    if discovering():

        def unavailable(_event: Any, _context: Any = None) -> None:
            raise RuntimeError("Lambda handlers do not run under TILDE_DISCOVERY=1")

        return unavailable
    initialize_tracing(tracing == "existing")
    initialize_logging(logging == "existing", deployment_logging)
    options = InvocationOptions(
        run=run,
        checkpoint=checkpoint,
        tracing=tracing,
        logging=logging,
        deployment_logging=deployment_logging,
    )
    host = HostState()

    def handler(event: Any, _context: Any = None) -> None:
        request = ParseDict(event, InvokeRequest(), ignore_unknown_fields=True)

        async def main() -> None:
            try:
                await run_invocation(request, options, host)
            except ConnectError as error:
                log.warning(
                    "invocation %s ended with an error: %s", request.invocation_id, error.message
                )

        asyncio.run(main())

    return handler
