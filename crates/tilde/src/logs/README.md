# Agent logs

`/v1/logs` accepts OTLP/HTTP JSON or protobuf on the agent runtime listener.
Invocation uploads share connect-token authorization and five-minute terminal
upload grace with traces. Deployment tokens also authorize logs (not traces),
including startup/background logs without any active invocation. Gateway lookup
rejects rotated tokens, retired deployments and deleted agents. Management
credentials never authorize ingestion. Rotel validates before batching, then Tilde
replaces all reserved `tilde.*` attributes with server-authored ownership:
`tilde.log.scope=invocation` with agent/invocation/run/thread IDs, or
`tilde.log.scope=deployment` with agent/deployment IDs. Private attribution headers
are stripped before authorization. Log records may omit trace/span IDs; supplied
invocation trace IDs must match the authenticated invocation trace. Deployment
logs may carry independent valid OTel trace/span correlation.
This accepts instrumented OTel logs; it does not automatically capture stdout.

`LOGS_CLICKHOUSE_URL`, `LOGS_CLICKHOUSE_DATABASE`, `LOGS_CLICKHOUSE_USER`, and
`LOGS_CLICKHOUSE_PASSWORD` configure history. Create the database first; Tilde
creates `otel_logs` using queries/_logs/schema.sql. The schema uses Rotel's Map-based
OTel layout and materialized AgentId, InvocationId, ThreadId, and LogId columns.
ReplacingMergeTree and FINAL queries collapse retries with the same normalized log
identity. Identical records at the same timestamp are considered one event; give
separate occurrences distinct timestamps. Missing timestamps use receiver time and
therefore cannot promise retry deduplication. Retention is seven days. The query
layer follows HyperDX's schema-agnostic OpenTelemetry source mapping: Timestamp,
Body, SeverityText, and ServiceName, with Tilde's ownership columns added.

The existing Rotel ClickHouse exporter writes compressed RowBinary batches with
async inserts and `wait_for_async_insert=1`. Logs do not pass through Postgres.
Each gateway uses `LOGS_QUEUE_DIR` for unencrypted protobuf delivery files. A process
lock requires one persistent directory per gateway replica. Acceptance follows
file and directory fsync, before remote delivery. Local history and optional
`LOGS_OTLP_ENDPOINT` forwarding have separate queues and retry loops. The external
endpoint must include its logs path and accept OTLP/HTTP protobuf. Optional
`LOGS_OTLP_HEADERS` uses comma-separated name=value pairs and stays on the gateway.

Delivery is at least once. Queue limits are 256 MiB for ClickHouse, 64 MiB for the
external collector, 32 MiB per agent per destination, and 4096 files per queue.
Undelivered batches expire after 24 hours with diagnostics. A full local queue
rejects uploads with a retryable response; it never blocks invocation RPCs. A full
external queue drops the external copy when local storage accepted it, with a
warning. In forwarding-only mode, a full external queue rejects ingestion instead.
Failed destinations retry independently. Disabling a destination discards its
backlog on restart; changing its endpoint/identity also discards the old backlog.
Queue limits bound disk and memory use, not the storage capacity of ClickHouse.

No destinations means authenticated, validated blackhole ingestion. External-only
configuration forwards logs while the native history controls remain disabled.
Configured ClickHouse outages show Unavailable and retain pending batches within
the stated limits. Malformed requests fail even when storage is disabled.

The management-only LogsService uses parameterized SQL with a mandatory server-side
agent predicate, bounded response sizes, eight concurrent history reads, and
30-day maximum query ranges. No arbitrary SQL or database credentials reach the
browser. `GetLogMetrics` aggregates the full matching window with the same filters
and `FINAL` deduplication as history; error counts include OTel severity 17 and above.

The Logs tab shares the tracing chip editor, dense date/time controls and histogram
brush. Its full-width IBM Plex Mono table has Time, Level and Messages columns,
28px virtual rows, and automatic cursor loading on scroll. The count histogram
marks intervals containing errors red and has no latency series. Brushing and
resizing its edges filters the table without changing the chart's horizon; full
width restores the original range. Explicit date changes replace the horizon.
Clearing dates restores the last 24 hours while preserving other filters.

Scope (`deployment` or `invocation`), message, service, session (thread), run, invocation, trace and minimum-severity
filters remain in the URL. Session/run filters use the server-authored ThreadId
and `tilde.run.id` attributes; table and histogram apply the same predicates.
Scope predicates use the presence of the server-authored InvocationId, so historical
invocation records remain discoverable without rewriting stored data.
Selecting a row opens escaped message/metadata details in a right-side sheet;
trace links open the native trace inspector. Live mode refreshes the newest 100
records every three seconds. It is a bounded live view, not a lossless subscription:
pause and query a fixed time interval to investigate high-volume gaps. History
scrolling retains loaded pages while rendering only the visible rows.

Sidecars queue immutable OTLP log batches in their bounded in-memory outbox and ship
them to the gateway in the publish stream. Only enablement is replicated to sidecars.
They retain cached settings while disconnected, discard when known disabled, and drop
batches after durable gateway acceptance. Gateway acceptance supplies ownership from
the authenticated deployment and does not reuse expired agent tokens.

Do preserve persistent queue volumes across restarts. Do scope every history query
by verified agent ID. Do monitor queue rejection, expiration and forwarding-loss
warnings. Do not expose ClickHouse or arbitrary query endpoints to agent callers.
Do not interpret successful local acceptance as guaranteed external delivery.

## HyperDX query basis

Query patterns are adapted from HyperDX (MIT), pinned at
[`c98be91`](https://github.com/hyperdxio/hyperdx/tree/c98be91f618fca6931ae0aea75553ea8b6573551):

- `packages/app/src/DBSearchPage.tsx`: histogram counts grouped by severity using the
  same exact search range as the result table.
- `packages/common-utils/src/core/renderChartConfig.ts`: `toStartOfInterval` buckets.
- `packages/common-utils/src/core/utils.ts`: automatic granularity targeting 60 buckets.
- `packages/common-utils/src/queryParser.ts`: literal, escaped `ILIKE` message search.

Tilde adapts these patterns to its typed filters and existing OTel Map schema. It
retains mandatory server-side agent scope, parameter binding, FINAL retry
deduplication, nanosecond stable cursors, query limits, and matching half-open
`[from,to)` bounds for the table and histogram. It does not expose arbitrary SQL.
