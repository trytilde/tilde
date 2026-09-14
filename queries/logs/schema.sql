-- Rotel's OTel Map schema, with materialized ownership columns for native queries.
-- Credentials can select a dedicated database; identifiers are never interpolated from requests.
CREATE TABLE IF NOT EXISTS otel_logs (
 Timestamp DateTime64(9) CODEC(Delta, ZSTD(1)),
 TraceId String CODEC(ZSTD(1)), SpanId String CODEC(ZSTD(1)), TraceFlags UInt8,
 SeverityText LowCardinality(String), SeverityNumber UInt8,
 ServiceName LowCardinality(String), Body String CODEC(ZSTD(3)),
 ResourceSchemaUrl String, ResourceAttributes Map(LowCardinality(String), String),
 ScopeSchemaUrl String, ScopeName String, ScopeVersion String,
 ScopeAttributes Map(LowCardinality(String), String),
 LogAttributes Map(LowCardinality(String), String), EventName String DEFAULT LogAttributes['tilde.log.event_name'],
 AgentId String MATERIALIZED LogAttributes['tilde.agent.id'],
 InvocationId String MATERIALIZED LogAttributes['tilde.invocation.id'],
 ThreadId String MATERIALIZED LogAttributes['tilde.thread.id'],
 LogId String MATERIALIZED LogAttributes['tilde.log.id'],
 INDEX trace_idx TraceId TYPE bloom_filter(0.01) GRANULARITY 1,
 INDEX invocation_idx InvocationId TYPE bloom_filter(0.01) GRANULARITY 1,
 INDEX body_idx Body TYPE tokenbf_v1(32768, 3, 0) GRANULARITY 4
) ENGINE = ReplacingMergeTree
PARTITION BY toDate(Timestamp)
ORDER BY (AgentId, Timestamp, LogId)
TTL toDateTime(Timestamp) + INTERVAL 7 DAY DELETE
SETTINGS index_granularity = 8192
