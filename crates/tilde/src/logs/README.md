# Agent logs

`/v1/logs` accepts OTLP/HTTP JSON or protobuf on the agent runtime listener.
It shares connect-token authorization and five-minute terminal upload grace with
traces. Management credentials never authorize ingestion. Rotel decodes and
validates the complete request before batching, then Tilde replaces ownership
attributes with verified agent, invocation, run, and thread IDs. Log records may
omit trace/span IDs; supplied trace IDs must match an available invocation trace.
This accepts instrumented OTel logs; it does not automatically capture stdout.

`LOGS_CLICKHOUSE_URL`, `LOGS_CLICKHOUSE_DATABASE`, `LOGS_CLICKHOUSE_USER`, and
`LOGS_CLICKHOUSE_PASSWORD` configure history. Create the database first; Tilde
creates `otel_logs` using queries/logs/schema.sql. The schema uses Rotel's Map-based
OTel layout and materialized AgentId, InvocationId, ThreadId, and LogId columns.
ReplacingMergeTree and FINAL queries collapse retries with the same normalized log
identity. Identical records at the same timestamp are considered one event; give
separate occurrences distinct timestamps. Missing timestamps use receiver time and
therefore cannot promise retry deduplication. Retention is seven days. HyperDX can
connect using source column mappings; its newer JSON/full-text schema is not
required. Tilde owns this table's schema rather than migrating HyperDX tables.

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
browser. The Logs tab offers severity, message, service, invocation, trace, and
time filters; cursor pagination; column preferences; and escaped record details.
Trace links open the native trace inspector. Live mode refreshes the newest 100
records every three seconds. It is a bounded live view, not a lossless subscription:
pause and query a fixed time interval to investigate high-volume gaps. History
browsing caps the browser at 1000 records until the filters are narrowed.

Sidecars queue immutable OTLP log batches in their bounded in-memory outbox and ship
them to the gateway in the publish stream. Only enablement is replicated to sidecars.
They retain cached settings while disconnected, discard when known disabled, and drop
batches after durable gateway acceptance. Gateway acceptance supplies ownership from
the authenticated deployment and does not reuse expired agent tokens.

Do preserve persistent queue volumes across restarts. Do scope every history query
by verified agent ID. Do monitor queue rejection, expiration and forwarding-loss
warnings. Do not expose ClickHouse or arbitrary query endpoints to agent callers.
Do not interpret successful local acceptance as guaranteed external delivery.
