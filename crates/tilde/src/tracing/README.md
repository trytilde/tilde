# Langfuse observability

Tilde embeds Rotel for standard agent OTLP ingestion and platform span batching.
Langfuse owns trace history, model/token analytics and observation queries. The
Postgres trace-history draft is not part of this implementation.

## Configuration and interfaces

Set LANGFUSE_BASE_URL, LANGFUSE_PUBLIC_KEY and LANGFUSE_SECRET_KEY on the gateway.
Keys identify one Langfuse project. LANGFUSE_PUBLIC_URL optionally supplies the
browser-facing address when the API uses an internal origin; path prefixes are
preserved. No project credentials are sent to the browser or replicated to sidecars.
Incomplete configuration disables tracing. Valid uploads are authenticated,
decoded, validated and discarded successfully; disabled mode performs no payload
storage or external network requests. Existing pending payloads are discarded on
startup in disabled mode. Invalid URL syntax is a configuration error.

Configured Langfuse outages/authorization errors produce an Unavailable viewer
state and retain pending delivery. Ingestion uses OTLP/HTTP protobuf and Langfuse's
v4 ingestion header. Reads use Observations API v2, never private tRPC endpoints.
The generic ENGINE_TRACING_EXPORT_* environment options are replaced by Langfuse
configuration.

The management-only TracingService exposes status, observation pages, trace pages
and session pages. Every read verifies the agent exists. Lists include its server-authored metadata
filter; trace details first prove agent membership, then include that trace's full
context so gateway parents and cooperating agents remain visible. Filter expressions are composed as one AND list because
Langfuse's advanced filter takes precedence over other query parameters. I/O text
search uses the supported `matches` operator. Initial pages omit the cursor;
subsequent pages retain the same filter and date bounds. Duplicate observation
versions are reconciled by trace/span identity and update time. Reads use a bounded,
ten-second cache, coalesce identical requests, and honor rate-limit cooldowns.

## Delivery and scope

Agent trace uploads remain on the agent runtime listener and use connect tokens.
The five-minute terminal upload grace does not restore any agent action permission.
Rotel accepts protobuf/JSON and gzip, validates whole requests before splitting,
and batches at 512 spans or 200 ms. Admission is bounded at 32 requests, with a
4 MiB decoded upload limit and a 15-second queue-acceptance deadline.

Enabled ingestion acknowledges a commit to `telemetry_delivery`, a temporary
outbox with a 256 MiB pending-payload cap. Queue payloads are raw protobuf, not
encrypted. Delivery deletes the payload while retaining its SHA-256 receipt for
seven days to deduplicate replay; neither the queue nor receipts support trace
history queries. A transaction lock makes capacity checks atomic. Full queues and
database failures apply backpressure and retryable errors without advancing a
receipt. The exporter wakes on Postgres notifications and scheduled retry deadlines.
Retries preserve payloads; authentication failures retry after a delay. Permanent
payload rejection/partial acceptance is terminal and reported without replaying
accepted subsets. Seven-day expiration bounds outstanding payloads and receipts.

Shutdown drains Rotel into the outbox, permits a bounded external flush, then
stops the worker. Remaining committed batches resume after restart. A crash after
Langfuse acceptance but before recording completion can deliver duplicates.

Mapping copies verified thread IDs to Langfuse session IDs and adds filterable
observation-level agent, invocation and run IDs. Trace/span IDs and parentage are
preserved. Supported OTel and AI SDK fields map to model, input/output, usage and
error fields. AI SDK wrapper totals are excluded so child generations are not
counted twice. Missing upstream metrics remain unavailable; Tilde does not invent
pricing. Request bodies and authorization headers are not captured by Tilde's HTTP
instrumentation. Telemetry ingestion, exporter requests and viewer reads do not
recursively create platform traces.

## Native viewer

The agent Tracing tab uses shadcn controls, React Hook Form and TanStack Table.
Observations and progressively reconstructed Sessions share UTC query bounds and
local timezone display, presets and text filters. Input/output data is rendered
as escaped text or expandable JSON. Column visibility is stored locally; filters
and selection live in the URL. Results are newest first, with cursor-based Load
more; partial session groups are explicitly labeled. There is no claim of global
session totals or arbitrary sorting unsupported by Langfuse's public API.

The inspector loads the span tree and timing bars progressively and identifies
parents outside loaded results. External links open the project, trace,
observation or session in a new tab without credentials. Known project links
remain available during a read outage. The read-only UI contains no feedback,
evaluation, prompt, dataset or administration write operations.

Selected formatting and I/O controls are adapted from Langfuse's MIT source;
`web/src/features/tracing/ATTRIBUTION.md` records the pinned source and license.

## Sidecar integration

The `codex/corrosion-sidecars` worktree carries the companion integration. Local
OTLP is batched with Rotel and committed to the existing Corrosion `traces` relation
as base64-encoded raw protobuf. It is not encrypted and never includes Langfuse keys.
The local relay is bounded at 64 MiB and receives only observability enablement
through gateway configuration. It retains the last known setting during outages.
Known-disabled sidecars discard valid payloads; disabling clears the local relay.

The gateway projects replicated batches into the same temporary outbox, then
removes acknowledged sidecar records. Stable payload identities deduplicate HA
and restart replay. Already accepted records use authenticated replication-group
ownership rather than expired invocation tokens. Both sidecar and gateway enforce
terminal upload grace on new agent requests.

Do preserve API scope and IDs when adapting instruments. Do treat queued delivery
as at least once. Do not use trace metadata to grant permissions, send project
keys to agents, fetch an entire project to filter it in the browser, or mistake
partial session pages for complete aggregate results.
