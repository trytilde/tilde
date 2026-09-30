-- Rotel's OTel span layout, with Tilde's ownership and observation facts materialized for queries.
-- One append-only table: a trace is its spans, and session and ownership facts ride on every row,
-- so reads never join. Retried deliveries of a span collapse on the sorting key.
-- Credentials can select a dedicated database; identifiers are never interpolated from requests.
CREATE TABLE IF NOT EXISTS otel_traces (
 Timestamp DateTime64(9) CODEC(Delta, ZSTD(1)),
 TraceId String CODEC(ZSTD(1)), SpanId String CODEC(ZSTD(1)), ParentSpanId String CODEC(ZSTD(1)),
 TraceState String CODEC(ZSTD(1)),
 SpanName LowCardinality(String) CODEC(ZSTD(1)), SpanKind LowCardinality(String) CODEC(ZSTD(1)),
 ServiceName LowCardinality(String) CODEC(ZSTD(1)),
 ResourceAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 ScopeName String CODEC(ZSTD(1)), ScopeVersion String CODEC(ZSTD(1)),
 SpanAttributes Map(LowCardinality(String), String) CODEC(ZSTD(3)),
 Duration UInt64 CODEC(ZSTD(1)),
 StatusCode LowCardinality(String) CODEC(ZSTD(1)), StatusMessage String CODEC(ZSTD(1)),
 Events Nested (Timestamp DateTime64(9), Name LowCardinality(String), Attributes Map(LowCardinality(String), String)) CODEC(ZSTD(1)),
 Links Nested (TraceId String, SpanId String, TraceState String, Attributes Map(LowCardinality(String), String)) CODEC(ZSTD(1)),
 AgentId String MATERIALIZED SpanAttributes['tilde.agent.id'],
 InvocationId String MATERIALIZED SpanAttributes['tilde.invocation.id'],
 ThreadId String MATERIALIZED SpanAttributes['tilde.thread.id'],
 ObservationType LowCardinality(String) MATERIALIZED upper(SpanAttributes['tilde.observation.type']),
 Level LowCardinality(String) MATERIALIZED if(SpanAttributes['tilde.observation.level'] = '', 'DEFAULT', SpanAttributes['tilde.observation.level']),
 Model LowCardinality(String) MATERIALIZED SpanAttributes['tilde.observation.model'],
 INDEX trace_idx TraceId TYPE bloom_filter(0.01) GRANULARITY 1,
 INDEX thread_idx ThreadId TYPE bloom_filter(0.01) GRANULARITY 1,
 INDEX invocation_idx InvocationId TYPE bloom_filter(0.01) GRANULARITY 1,
 INDEX attribute_key_idx mapKeys(SpanAttributes) TYPE bloom_filter(0.01) GRANULARITY 1,
 INDEX input_idx SpanAttributes['tilde.observation.input'] TYPE tokenbf_v1(32768, 3, 0) GRANULARITY 4,
 INDEX output_idx SpanAttributes['tilde.observation.output'] TYPE tokenbf_v1(32768, 3, 0) GRANULARITY 4
) ENGINE = ReplacingMergeTree
PARTITION BY toDate(Timestamp)
ORDER BY (AgentId, Timestamp, TraceId, SpanId)
SETTINGS index_granularity = 8192
