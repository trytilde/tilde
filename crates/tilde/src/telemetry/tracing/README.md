# Tracing

Tilde embeds Rotel for standard agent OTLP ingestion and platform span batching, stores
spans in its own ClickHouse table, and can forward every batch to an external OTLP/HTTP
collector. There is no third-party trace backend.

## Configuration and interfaces

History uses the same ClickHouse database as logs (`ENGINE_CLICKHOUSE_*`); Tilde creates
`otel_traces` from `queries/_traces/schema.sql`. ClickHouse and `ENGINE_TRACES_S3_BUCKET`
are required; the bucket is where accepted batches wait, and it
needs a lifecycle rule expiring objects after at least a day, since the engine never
deletes them. `ENGINE_MEDIA_S3_BUCKET` holds media and oversized payloads for as long as
spans reference them and must not carry that expiry; without it both stay inline.
`TRACES_OTLP_ENDPOINT` must include its traces path and accept OTLP/HTTP protobuf;
`TRACES_OTLP_HEADERS` uses comma-separated name=value pairs and stays on the gateway. No
storage credentials are sent to the browser or replicated to sidecars. ClickHouse is assumed
to be running, so the viewer reports no storage state; during an outage its queries fail and
pending delivery is retained within the queue limits. A full queue refuses new batches before
uploading them, and telemetry uses its own Postgres pool.

The span table is Rotel's OpenTelemetry layout, so its exporter writes it unchanged and
standard OpenTelemetry tooling can read it. Tilde adds materialized columns read from span
attributes (AgentId, InvocationId, ThreadId, ObservationType, Level, Model), bloom-filter
indexes on trace, thread and invocation ids and on attribute keys, token indexes on
observation input and output, and a sorting key of (AgentId, Timestamp, TraceId, SpanId).
Retention (`ENGINE_CLICKHOUSE_RETENTION_DAYS`) is applied at start-up; when unset, an existing TTL is
removed and a failure to do so blocks initialization rather than silently keeping the old
policy. It is append-only: ReplacingMergeTree on the sorting key collapses retried
deliveries, and reads use FINAL. Trace-level facts ride on every span, so no read joins.

The management-only TracingService exposes status, observation pages, trace pages,
session pages, and full-range activity metrics. Every read verifies the agent exists and
carries a server-authored agent predicate; trace details first prove agent membership,
then read that trace's full context, bounded to a day either side of the agent's own
spans, so gateway parents and cooperating agents remain visible. Filters are
conditions on named columns (`viewer/filter.rs`): columns and operators are matched against fixed
vocabularies and rendered into `queries/_traces` templates, every value is a query
parameter, and text operators are escaped to literal substrings. Metadata conditions name a
span attribute and take the string operators, or the number operators. Pages are 50 rows, newest first,
with a keyset cursor bound to its filter identity and time window. Ranges are at most 90
days, results at most 32 MiB, and eight reads run at once.

## Delivery and scope

Agent trace uploads remain on the agent runtime listener and use connect tokens.
The five-minute terminal upload grace does not restore any agent action permission.
Rotel accepts protobuf/JSON and gzip, validates whole requests before splitting,
and batches at 512 spans or 200 ms. Admission is bounded at 32 requests, with a
4 MiB decoded upload limit and a 15-second queue-acceptance deadline.

Enabled ingestion acknowledges once the batch is in the trace bucket and its pointer is
committed to `telemetry_objects`; see `crate::telemetry::spool`. A batch is identified by the SHA-256
of its payload before session enrichment, so a retried upload is not queued twice while the
first copy is pending; each enqueue still writes its own object, so completing one delivery
never deletes a copy a retry queued meanwhile. Admission holds a per-queue advisory lock
around the capacity read and pointer insert, so concurrent accepts cannot exceed the limits.
Payloads are raw protobuf, not encrypted beyond the bucket's own settings. Each destination
has its own queue (`traces/clickhouse`, `traces/external`) with pending limits per queue and
per agent; full queues and storage failures apply backpressure and retryable errors. Any
replica claims a pointer under a 45-second lease; success removes the pointer, failure
releases it with exponential backoff, and a crashed holder's lease simply runs out. The
object is never deleted by the engine; the bucket lifecycle rule expires it. Delivery is
therefore at least once. The sweeper drops pointers older than 24 hours from every queue.
Permanent payload rejection or partial acceptance by a collector is terminal and reported without replaying accepted subsets.

Shutdown drains Rotel into the queue, then stops the workers. Committed batches are
delivered by whichever replica runs next.

The gateway projects observation facts under `tilde.observation.*` before queueing: type
from span names and `gen_ai.operation.name`, model, input and output from AI SDK and
GenAI semantic-convention attributes, level from span status, and time to first token.
Thread ids are sessions. Trace and span ids and parentage are preserved. AI SDK wrapper
totals are excluded so child generations are not counted twice. Tilde does not invent
pricing: `crate::pricing` stamps `tilde.observation.cost_usd` and `.provider` on generation
spans the price table covers, from the same rates that price inference requests. Media
and oversized payloads in any span attribute, the projected fields and their sources alike,
are moved to the media bucket by `media.rs` before queueing. Request
bodies and authorization headers are not captured by Tilde's HTTP instrumentation.
Telemetry ingestion, delivery and viewer reads do not recursively create platform traces.

## Native viewer

A compact activity chart between the filters and table shows full-range observation
counts with an average-latency line (seconds). Intervals containing errors are red.
`GetObservationMetrics` runs one span-store query with the same agent and observation
filters as the table, including input and output search, grouped into calendar buckets in
UTC. Duration sums and counts produce a weighted mean; empty intervals have no latency.
Queries are bounded and fail visibly instead of presenting partial totals. The chart supports forward/reverse range brushing, resizable selection edges,
Escape cancellation, and keyboard interval/edge selection. Brushing preserves the
chart horizon and aggregates while updating the table URL time filter. Expanding
the selection to full width clears the brush and restores its original range.
Explicit date-control changes replace the horizon; other filters refresh its data. Clearing the date range restores the
default last 24 hours and preserves the other filters.

The agent Tracing tab uses shadcn controls, React Hook Form, TanStack Table v9 and
TanStack Virtual. Its compact IBM Plex Mono filter bar contains type selectors,
a field:value autocomplete, date range and refresh. The
query editor suggests supported fields, span attributes and observed values, accepts
any dotted span attribute as an exact-match filter, accepts quoted values,
and applies through the existing URL/server filters on Enter. Unsupported syntax
is rejected without changing the active query. Plain text searches names.

Filtering and 50-row cursor requests remain server-owned. Scrolling loads
one further block at a time; only viewport rows plus overscan render. Query changes
abort the previous stream. The browser deduplicates observation versions across
blocks and derives trace metrics from those loaded records. Sessions is a separate
header tab after IAM and reads the same trace data. `include_session_details` requests
ID-to-label resolution for display; it does not query a separate analytics table.

The existing delivery queue captures session metadata at the gateway after ownership
normalization. It adds `tilde.session.provider_id`, `provider_name`, `provider_icon_url`,
`connection_id`, `identity_ids`, `message_count`, `last_turn_time`, and `metadata_time`.
Counts include
retained nonempty or attachment-only messages. Last turn is the latest real message or
invocation start. The metadata query runs once per accepted batch, scoped to its distinct
authenticated agent/session pairs. No identity values or message bodies are added.

Reserved session fields are stripped before delivery receipt hashing and replaced from
trusted state. Replays of either the original or enriched batch reuse the same receipt,
so delivery retries do not refresh counters or timestamps. Session summaries choose the
newest captured metadata tuple, including lower counts after deletions; they do not sum
counts across spans. PostgreSQL only resolves captured identity IDs on the read path.
Unrecorded fields stay unknown, and idle sessions do not gain live rollups without new
traces. Run `task test:session-metadata` to exercise capture, replay, ownership and reads.

The abandoned session projection has no runtime dependencies. Its PostgreSQL objects
are dropped by the cleanup migration. Existing installations that ran the experiment
can remove its ClickHouse table using `queries/_migrations/remove_session_clickhouse.sql`
in the former ENGINE_CLICKHOUSE_DATABASE; normal log and trace tables are unaffected.

The inspector is a right-side shadcn Sheet. Microcharts TraceFold renders real span
batches of at most 40 on a shared time scale; a long trace is not truncated by the
library's cap. The full-width top chart has a 500ms axis/grid (labels thin out for
long traces) and initially fits the whole time range horizontally. Zoom changes the
millisecond scale only: rows remain 26px and labels 11px. Long sessions scroll
vertically at native speed, independently of zoom. Trackpad pinch (including
Safari GestureEvents), native two-finger scrolling, mouse drag, and zoom/fit
buttons share this timeline model. Zoom keeps the time under the pointer and the
vertical reading position stable. Labels remain anchored to span starts. Trailing
time, label overhang, and pan padding keep final spans accessible; controls float
at the top right. Microcharts SVGs use explicit heights and independent axis
sizing so wide time ranges cannot shrink the rows. More spans load near the bottom or through the
load-more control. Hovering anywhere on a row previews its details; leaving restores the clicked selection. The span-count/total-time
header stays fixed. An arrow-ended duration ruler beneath the active span shows
its duration in milliseconds, floating below the unchanged bar. It has a light hover time-band
highlight, chart labels that turn bold on row hover, keyboard/click selection, and orange (500–1000ms) / red (>1000ms) timing
colors. Input, Output and Metadata tabs show the selected span. The header full-screen toggle expands to two
columns: the fitted graph on the left and the same details on the right. Formatted payloads recognize messages,
render Markdown safely without fetching remote images, and retain exact JSON as
an alternate view. Search terms render as editable/removable chips; hover or focus
expands overflow above the table without changing its position. Date options open
below the trigger. Custom ranges use shadcn Calendar/Popover date pickers
(DD/MM/YY) paired with compact 24-hour time inputs, including seconds; valid edits
apply immediately. Filters and
inspection selections remain shareable in the URL.

Grid durations use seconds with three decimals; latency over one second is red.
Dates use DD/MM/YY HH:mm:ss in the browser timezone. First-token timing appears
only when captured. External project/trace/observation links never contain API keys.

## Sidecar integration

A sidecar batches local OTLP with Rotel, stamps each span with the verified invocation
scope, and queues the batch as a telemetry frame in its publish stream to the gateway.
Nothing is written to disk and no storage credentials reach the sidecar. Enablement arrives
through gateway configuration; known-disabled sidecars discard valid payloads.

The gateway re-stamps the agent from the authenticated deployment token and accepts the
batch into the same temporary outbox. A batch is only dropped from sidecar memory once
the gateway acknowledged the publish. Both sidecar and gateway enforce terminal upload
grace on new agent requests.

Do preserve API scope and IDs when adapting instruments. Do treat queued delivery
as at least once. Do not use trace metadata to grant permissions, send project
keys to agents, fetch an entire project to filter it in the browser, or mistake
partial session pages for complete aggregate results.

## Component versions

New SDK spans carry `tilde.client_sdk.version`, read from the installed SDK's
package.json. A sidecar stamps `tilde.sidecar.version` when queueing agent or platform
spans, and the gateway stamps `tilde.gateway.version` before its durable queue.
Server versions use the running binary's Cargo package version. Retries replay the
stored versions rather than relabeling earlier batches after an upgrade.

Ownership normalization retains the SDK version and replaces server-owned values.
Versions are stored with the span as `tilde.client_sdk.version`, `tilde.sidecar.version`
and `tilde.gateway.version`, and appear in the inspector's metadata. Direct traces
omit sidecar provenance; gateway-only traces without an SDK omit client provenance.
Existing stored traces are unchanged.
