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

The management-only TracingService exposes status, observation pages, trace pages,
session pages, and full-range activity metrics. Every read verifies the agent exists. Lists include its server-authored metadata
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

A compact activity chart between the filters and table shows full-range observation
counts with an average-latency line (seconds). Intervals containing errors are red.
`GetObservationMetrics` uses the same agent and observation filters as the table,
with Langfuse v2 metrics grouped by time/level. Sums and non-null latency counts
produce a weighted mean; empty intervals have no latency. Content searches use
minimal v2 observation pages and deduplicate versions across pages, since metrics
cannot execute input/output `matches`. Scans are bounded and fail visibly instead
of presenting partial totals. The chart supports forward/reverse range brushing, resizable selection edges,
Escape cancellation, and keyboard interval/edge selection. Brushing preserves the
chart horizon and aggregates while updating the table URL time filter. Expanding
the selection to full width clears the brush and restores its original range.
Explicit date-control changes replace the horizon; other filters refresh its data. Clearing the date range restores the
default last 24 hours and preserves the other filters.

The agent Tracing tab uses shadcn controls, React Hook Form, TanStack Table v9 and
TanStack Virtual. Its compact IBM Plex Mono filter bar contains type selectors,
a field:value autocomplete, date range, refresh and a direct Langfuse link. The
query editor suggests supported fields and observed values, accepts quoted values,
and applies through the existing URL/server filters on Enter. Unsupported syntax
is rejected without changing the active query. Plain text searches names.

Langfuse filtering and 50-row cursor requests remain server-owned. Scrolling loads
one further block at a time; only viewport rows plus overscan render. Query changes
abort the previous stream. The browser deduplicates observation versions across
blocks and derives trace metrics from those loaded records. Sessions is a separate
header tab after IAM and reads the same trace data. `include_session_details` requests
ID-to-label resolution for display; it does not query a separate analytics table.

The existing delivery queue captures session metadata at the gateway after ownership
normalization. It adds `tilde.session.provider_id`, `provider_name`, `provider_icon_url`,
`connection_id`, `identity_ids`, `message_count`, `last_turn_time`, and `metadata_time`,
also mapped to `langfuse.{observation,trace}.metadata.tilde_session_*`. Counts include
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
in the former LOGS_CLICKHOUSE_DATABASE; normal log and trace tables are unaffected.

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

Selected formatting and I/O controls are adapted from Langfuse's MIT source;
`web/src/features/tracing/ATTRIBUTION.md` records the pinned source and license.

## Sidecar integration

A sidecar batches local OTLP with Rotel, stamps each span with the verified invocation
scope, and queues the batch as a telemetry frame in its publish stream to the gateway.
Nothing is written to disk and no Langfuse keys reach the sidecar. Enablement arrives
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
Projection maps versions to `tilde_client_sdk_version`, `tilde_sidecar_version` and
`tilde_gateway_version` in Langfuse observation and trace metadata. Direct traces
omit sidecar provenance; gateway-only traces without an SDK omit client provenance.
Existing stored traces are unchanged.
