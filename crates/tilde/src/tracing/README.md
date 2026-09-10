# Tracing

Tilde embeds the pinned Rotel library in `vendor/rotel`. Its OTLP receiver and
batch pipeline forward platform and agent spans to an external collector. Trace
payloads are not stored in Postgres or on disk.

## Configuration

Set `ENGINE_TRACING_EXPORT_ENDPOINT` to the complete OTLP/HTTP protobuf traces URL,
including `/v1/traces`. Optional `ENGINE_TRACING_EXPORT_HEADERS` supplies
comma-separated `name=value` headers, for example `Authorization=Bearer ...`.
Headers require an endpoint and are never logged. Without an endpoint, platform
tracing is disabled and the SDK skips trace export for unsampled invocations.
Authenticated direct uploads then receive 503 instead of being silently discarded.

## Ingestion and propagation

`POST /v1/traces` is exposed only on the agent runtime listener, outside the
ConnectRPC wire protocol. It accepts standard OTLP protobuf or JSON, including
gzip, using `Authorization: Bearer <agent connect token>`. Management credentials
cannot authorize ingestion. Middleware verifies invocation scope and replaces
internal headers; decoded requests must match the invocation's execution trace.
Agent-supplied `tilde.*` attributes are replaced with verified ownership.

Postgres remains the application's source for token validation and durable
message/invocation trace context. These are domain records, not stored spans.
W3C traceparent/tracestate propagates across webhook/API requests, queued messages,
invocation dispatch and SDK callbacks. HTTP/SDK response spans remain open until
their streams end or are dropped. Completed spans are exported during execution;
standard OTel does not emit updates to unfinished spans. Frameworks must use OTel
instrumentation for their model/tool spans to be collected.

## Batching and delivery

Rotel batches up to 512 spans or 200 ms. Input and export queues hold 32 and 8
batches. Ingress permits 32 concurrent requests and limits decoded requests to
4 MiB; queue admission/upload has a 15-second deadline. Full admission or an
expired deadline returns retryable 503 with Retry-After. Success acknowledges
acceptance into memory, not delivery to the external collector.

The exporter processes bounded batches asynchronously and honors retryable OTLP
statuses and Retry-After. Exponential backoff starts at one second and caps at
30 seconds; each batch has a five-minute retry budget. Exhausted retries and
permanent/partial rejections drop the affected batch with a diagnostic. A slow
collector fills the bounded queues and applies backpressure to ingestion. SDK
buffers are also finite (4096 platform spans and 2048 spans per agent invocation).
There is no local replay or deduplication; the destination must tolerate duplicates
from ambiguous responses. Process failure loses telemetry still held in memory.

Shutdown flushes the platform SDK and drains Rotel toward the collector within a
bounded deadline. A collector outage during shutdown can lose queued spans. OTLP
ingestion and the exporter do not recursively generate platform traces.

## Token lifecycle

The same connect token authorizes trace uploads while the invocation is live and
for five minutes after `ended_at`, including failure and cancellation. A JWT that
expires within this window qualifies only if it was valid when the invocation
ended. Other agent actions and renewal still require a live invocation and an
unexpired token. The SDK batches separately per invocation and reads its current
renewed token at export time; final upload does not reuse the canceled work signal.

Run `task test:tracing` for collector forwarding, retries, context propagation,
listener authentication and SDK streaming checks. Postgres trace persistence is
preserved separately for a follow-up branch/PR.
