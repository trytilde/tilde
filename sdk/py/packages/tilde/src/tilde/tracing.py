"""Invocation-scoped OTel tracing: W3C context from the wake, spans uploaded to Tilde."""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass

from opentelemetry import context as otel_context
from opentelemetry import propagate, trace
from opentelemetry.exporter.otlp.proto.common.trace_encoder import encode_spans
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import ReadableSpan, Span, SpanProcessor, TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor, SpanExporter, SpanExportResult
from opentelemetry.sdk.trace.sampling import ALWAYS_ON
from opentelemetry.trace import SpanKind, Status, StatusCode

from tilde._otlp import otlp_url, post_protobuf

_SESSION = otel_context.create_key("tilde.invocation.telemetry")


class InvocationSpanProcessor(SpanProcessor):
    """Add this processor to an existing provider, then use ``tracing="existing"`` on the host."""

    def __init__(self) -> None:
        self._owners: dict[int, BatchSpanProcessor] = {}
        self._processors: set[BatchSpanProcessor] = set()

    def on_start(self, span: Span, parent_context: otel_context.Context | None = None) -> None:
        processor = otel_context.get_value(_SESSION, parent_context)
        if isinstance(processor, BatchSpanProcessor):
            self._owners[id(span)] = processor

    def on_end(self, span: ReadableSpan) -> None:
        processor = self._owners.pop(id(span), None)
        if processor is not None:
            processor.on_end(span)

    def add(self, processor: BatchSpanProcessor) -> None:
        self._processors.add(processor)

    def remove(self, processor: BatchSpanProcessor) -> None:
        self._processors.discard(processor)

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        return all(p.force_flush(timeout_millis) for p in list(self._processors))

    def shutdown(self) -> None:
        for processor in list(self._processors):
            processor.shutdown()
        self._processors.clear()


agent_span_processor = InvocationSpanProcessor()
_initialized = False


def initialize_tracing(existing: bool) -> None:
    global _initialized
    if existing or _initialized:
        return
    provider = TracerProvider(
        resource=Resource.create({"service.name": "tilde-agent"}), sampler=ALWAYS_ON
    )
    provider.add_span_processor(agent_span_processor)
    trace.set_tracer_provider(provider)
    _initialized = True


class _Exporter(SpanExporter):
    def __init__(self, url: str, authorization: Callable[[], str]) -> None:
        self._url = url
        self._authorization = authorization

    def export(self, spans: Sequence[ReadableSpan]) -> SpanExportResult:
        payload = encode_spans(spans).SerializeToString()
        ok = post_protobuf(self._url, payload, self._authorization)
        return SpanExportResult.SUCCESS if ok else SpanExportResult.FAILURE

    def shutdown(self) -> None:
        return None

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        return True


@dataclass(slots=True)
class InvocationTelemetry:
    context: otel_context.Context
    _end: Callable[[bool], None]

    async def end(self, failed: bool) -> None:
        await asyncio.to_thread(self._end, failed)


def invocation_tracing(
    request, headers: Mapping[str, str], authorization: Callable[[], str]
) -> InvocationTelemetry:
    """Extract the platform's context; export only when Tilde sampled the invocation."""
    traceparent = headers.get("traceparent", "")
    parts = traceparent.split("-")
    sampled = len(parts) == 4 and parts[3].isalnum() and int(parts[3], 16) & 1
    parent = propagate.extract(headers)
    if not sampled:
        # Preserve correlation for OTel logs even when trace export is disabled.
        return InvocationTelemetry(parent, lambda _failed: None)
    exporter = _Exporter(otlp_url(request.callback_url, "traces"), authorization)
    processor = BatchSpanProcessor(
        exporter,
        max_queue_size=2048,
        max_export_batch_size=256,
        schedule_delay_millis=200,
        export_timeout_millis=20000,
    )
    agent_span_processor.add(processor)
    parent = otel_context.set_value(_SESSION, processor, parent)
    span = trace.get_tracer("trytilde").start_span(
        "agent.invoke",
        context=parent,
        kind=SpanKind.SERVER,
        attributes={
            "tilde.invocation.id": request.invocation_id,
            "tilde.agent.id": request.agent_id,
            "tilde.run.id": request.run_id,
            "tilde.thread.id": request.thread_id,
        },
    )
    active = trace.set_span_in_context(span, parent)
    ended = False

    def end(failed: bool) -> None:
        nonlocal ended
        if ended:
            return
        ended = True
        if failed:
            span.set_status(Status(StatusCode.ERROR, "Agent invocation failed"))
        span.end()
        try:
            processor.shutdown()
        except Exception:  # noqa: BLE001 - tracing must not fail agent work
            pass
        finally:
            agent_span_processor.remove(processor)

    return InvocationTelemetry(active, end)
