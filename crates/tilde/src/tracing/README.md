# Tracing

Tilde embeds a pinned source fork of Rotel from `vendor/rotel`; no separate
collector process or listening port is required. See `vendor/rotel/TILDE.md` for
upstream provenance and the small receiver/batching changes.

## Data flow

Agent OTLP/HTTP -> token/admission middleware -> Rotel receiver -> bounded Rotel
batch pipeline -> Postgres. The platform OTel provider feeds the same pipeline.
An independent worker reads pending persisted spans and forwards OTLP/HTTP to the
optional external destination. External downtime does not hold ingestion open.

`iam::listeners::agent_runtime_router` owns agent ingestion; the management listener
factory has no upload route. Platform request instrumentation on management is distinct
from accepting agent traces.

`POST /v1/traces` is mounted on the agent runtime listener, outside the ConnectRPC
wire protocol. It accepts standard OTLP protobuf or JSON (and gzip), authenticated
with `Authorization: Bearer <agent connect token>`. It has no management/OIDC
credential fallback. Authentication replaces internal scope headers; decoded
requests are checked against the invocation's execution trace before batching.
Agent-supplied `tilde.*` span attributes are replaced with verified ownership.

Request middleware creates server spans without recording headers, bodies, query
strings or raw URL parameters. W3C traceparent/tracestate is stored with messages
and pending invocations, so webhook/API ancestry survives worker scheduling and
restarts. Execution creates a child invocation span and injects its context into
the agent RPC. SDK model/framework spans and callback RPCs share that context.
HTTP and SDK response spans end when streams end or are dropped, not when response
headers arrive. Completed spans flush continuously during execution; standard OTel
export does not emit updates to unfinished spans. OTLP uploads and the forwarding worker do not generate recursive
telemetry. Frameworks must use OTel instrumentation; Tilde cannot reconstruct
unrecorded model/tool spans.

## Persistence and delivery

Rotel batches up to 512 spans or 200 ms. Ingress is limited to 32 concurrent
requests and 4 MiB per decoded request; the input and database queues hold 32 and
8 batches respectively. Requests wait up to 15 seconds for queue admission and
persistence; saturation/timeouts return retryable 503 responses. HTTP success
means the batch's spans have committed, including when Rotel splits the request.
Postgres outages retain the current batch and apply backpressure. Platform and
agent OTel SDK queues are finite (4096 and 2048 spans respectively); sustained
overload can exhaust SDK buffers. These in-memory queues are not a crash journal.

`tracing_spans` deduplicates on trace/span ID and indexes invocation, parent and
nanosecond timing fields. Each row preserves one complete OTLP resource/scope/span
envelope, including events, links, typed attributes and schema URLs, as raw
protobuf bytes. Retries never overwrite a stored span.

External delivery is at least once. A worker locks eligible rows with SKIP LOCKED,
streams at most 256 spans per attempt from Postgres, and sends bounded OTLP/HTTP requests. Retryable
failures use persisted exponential backoff and Retry-After; permanent/partial
rejections are recorded without replaying an already accepted subset. Sending
and recording success cannot be atomic across HTTP and Postgres, so a crash after
remote acceptance may deliver duplicates. The external backend must tolerate that.

Retention is seven days by default and cleanup runs hourly. It also removes old
pending deliveries, bounding the forwarding backlog by retention. Pending rows
use the current configured destination after restart; changing the environment
redirects that backlog. Spans collected with forwarding disabled are stored with
`delivery='disabled'` and are not automatically backfilled when export is enabled.

Shutdown first closes application requests/workers, then flushes the platform SDK
and drains Rotel into Postgres. External pending work resumes after restart. The
drain is bounded; unacknowledged agent uploads must retry if shutdown interrupts it.

## Agent token lifecycle

Trace ingestion accepts the same connect token while the invocation is live and
until five minutes after recorded `ended_at`, including completion, failure and
cancellation. A JWT that expires within that terminal window is accepted only if
it was valid when the invocation ended, with at most five minutes of expiry grace.
Expired JWTs cannot upload for still-running invocations. All other agent actions
and renewal continue to require an unexpired token and a live invocation.

The TypeScript SDK maintains a separate batch processor per invocation and reads
its latest renewed token at export time. The exporter preserves the runtime callback
URL path prefix, so a proxied `/runtime` base uploads to `/runtime/v1/traces`. Concurrent invocations can share a trace
without mixing their credentials. Final upload does not use the canceled agent
signal and may complete after the invocation ends.

## Configuration and validation

- `ENGINE_TRACING_EXPORT_ENDPOINT`: optional exact OTLP/HTTP traces URL, including
  `/v1/traces`. HTTP protobuf forwarding only; use a collector to bridge to gRPC.
- `ENGINE_TRACING_EXPORT_HEADERS`: comma-separated `name=value` headers, for example
  `Authorization=Bearer ...`. Requires an endpoint; header values are never logged.
- `ENGINE_TRACING_RETENTION_DAYS`: 1–365, default 7.

Run `task test:tracing` for receiver/Postgres/export integration and SDK context,
credential rotation, and response-stream lifecycle tests. Trace UI rendering and
query APIs are not included in this change.
