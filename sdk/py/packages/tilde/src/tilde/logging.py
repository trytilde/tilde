"""Invocation and deployment scoped OTel logs. Routing is captured at emit time, before batching."""

from __future__ import annotations

import asyncio
import logging
import os
import warnings
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field

from opentelemetry import _logs as otel_logs
from opentelemetry import context as otel_context
from opentelemetry.exporter.otlp.proto.common._log_encoder import encode_logs
from opentelemetry.sdk._logs import (
    LoggerProvider,
    LoggingHandler,
    LogRecordProcessor,
    ReadWriteLogRecord,
)
from opentelemetry.sdk._logs.export import (
    BatchLogRecordProcessor,
    LogRecordExporter,
    LogRecordExportResult,
)
from opentelemetry.sdk.resources import Resource

from tilde._otlp import otlp_url, post_protobuf

_INVOCATION = otel_context.create_key("tilde.invocation.logs")


@dataclass(slots=True)
class DeploymentLogging:
    """Export startup/background logs with deployment credentials (one deployment per process)."""

    gateway_url: str
    deployment_token: str


@dataclass(slots=True, eq=False)
class _Session:
    processor: BatchLogRecordProcessor
    attributes: dict[str, str]
    closed: bool = False
    authorization: Callable[[], str] | None = field(default=None, repr=False)


class InvocationLogProcessor(LogRecordProcessor):
    """Attach to any OTel LoggerProvider; records outside a Tilde scope are dropped."""

    def __init__(self) -> None:
        self._sessions: set[_Session] = set()
        self._deployment: _Session | None = None
        self._deployment_config: DeploymentLogging | None = None

    def configure_deployment(self, options: DeploymentLogging) -> None:
        if self._deployment_config is not None:
            if self._deployment_config != options:
                raise RuntimeError("agent_log_processor supports one deployment per process")
            return
        token = options.deployment_token
        processor = _batch_processor(options.gateway_url, lambda: f"Bearer {token}")
        self._deployment_config = DeploymentLogging(options.gateway_url, options.deployment_token)
        self._deployment = _Session(
            processor=processor, attributes={"tilde.log.scope": "deployment"}
        )
        self._sessions.add(self._deployment)

    def on_emit(self, log_record: ReadWriteLogRecord) -> None:
        # A closed invocation stays closed: never reclassify its late logs as deployment logs.
        record = log_record.log_record
        session = otel_context.get_value(_INVOCATION, getattr(record, "context", None))
        if session is None:
            session = otel_context.get_value(_INVOCATION)
        if not isinstance(session, _Session):
            session = self._deployment
        if session is None or session.closed:
            return
        attributes = record.attributes
        if attributes is None:
            record.attributes = dict(session.attributes)
        else:
            try:
                attributes.update(session.attributes)
            except (AttributeError, TypeError):
                record.attributes = {**dict(attributes), **session.attributes}
        session.processor.on_emit(log_record)

    def add(self, session: _Session) -> None:
        self._sessions.add(session)

    def remove(self, session: _Session) -> None:
        self._sessions.discard(session)

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        return all(s.processor.force_flush(timeout_millis) for s in list(self._sessions))

    def shutdown(self) -> None:
        sessions = list(self._sessions)
        for session in sessions:
            session.closed = True
        try:
            for session in sessions:
                session.processor.shutdown()
        finally:
            self._sessions.clear()
            self._deployment = None
            self._deployment_config = None


agent_log_processor = InvocationLogProcessor()
_initialized = False


def configure_deployment_logging(gateway_url: str, deployment_token: str) -> None:
    """Configure before startup logs are emitted. One process owns one deployment."""
    agent_log_processor.configure_deployment(DeploymentLogging(gateway_url, deployment_token))


def initialize_logging(existing: bool, deployment: DeploymentLogging | None) -> None:
    """Default hosts route the standard ``logging`` module through OTel to Tilde."""
    global _initialized
    if deployment is not None:
        agent_log_processor.configure_deployment(deployment)
    if existing or _initialized:
        return
    provider = LoggerProvider(
        resource=Resource.create(
            {"service.name": os.environ.get("OTEL_SERVICE_NAME") or "tilde-agent"}
        )
    )
    provider.add_log_record_processor(agent_log_processor)
    otel_logs.set_logger_provider(provider)
    with warnings.catch_warnings():
        # The SDK handler is deprecated in favor of an instrumentation package without one.
        warnings.simplefilter("ignore", DeprecationWarning)
        handler = LoggingHandler(logger_provider=provider)
    logging.getLogger().addHandler(handler)
    _initialized = True


class _Exporter(LogRecordExporter):
    def __init__(self, url: str, authorization: Callable[[], str]) -> None:
        self._url = url
        self._authorization = authorization

    def export(self, batch: Sequence) -> LogRecordExportResult:
        payload = encode_logs(batch).SerializeToString()
        ok = post_protobuf(self._url, payload, self._authorization)
        return LogRecordExportResult.SUCCESS if ok else LogRecordExportResult.FAILURE

    def shutdown(self) -> None:
        return None

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        return True


def _batch_processor(base_url: str, authorization: Callable[[], str]) -> BatchLogRecordProcessor:
    return BatchLogRecordProcessor(
        _Exporter(otlp_url(base_url, "logs"), authorization),
        max_queue_size=2048,
        max_export_batch_size=256,
        schedule_delay_millis=200,
        export_timeout_millis=20000,
    )


@dataclass(slots=True)
class InvocationLogs:
    context: otel_context.Context
    _end: Callable[[], None]

    async def end(self) -> None:
        await asyncio.to_thread(self._end)


def invocation_logging(
    request, parent: otel_context.Context, authorization: Callable[[], str]
) -> InvocationLogs:
    session = _Session(
        processor=_batch_processor(
            request.callback_url, lambda: session.authorization() if session.authorization else ""
        ),
        attributes={
            "tilde.log.scope": "invocation",
            "tilde.agent.id": request.agent_id,
            "tilde.thread.id": request.thread_id,
            "tilde.run.id": request.run_id,
            "tilde.invocation.id": request.invocation_id,
        },
        authorization=authorization,
    )
    agent_log_processor.add(session)
    active = otel_context.set_value(_INVOCATION, session, parent)
    finished = False

    def end() -> None:
        nonlocal finished
        if finished:
            return
        finished = True
        session.closed = True
        try:
            session.processor.shutdown()
        except Exception:  # noqa: BLE001 - export failure must not fail the invocation
            pass
        finally:
            session.authorization = None
            agent_log_processor.remove(session)

    return InvocationLogs(active, end)
