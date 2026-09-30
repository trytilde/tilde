-- Rotel's OTel metrics layout, one table per data point kind, with the owning agent materialized
-- from the data point attributes the gateway stamps. Retention is applied at start-up.
CREATE TABLE IF NOT EXISTS otel_metrics_sum (
 ResourceAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)), ResourceSchemaUrl String CODEC(ZSTD(1)),
 ScopeName String CODEC(ZSTD(1)), ScopeVersion String CODEC(ZSTD(1)), ScopeAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 ScopeDroppedAttrCount UInt32 CODEC(ZSTD(1)), ScopeSchemaUrl String CODEC(ZSTD(1)),
 ServiceName LowCardinality(String) CODEC(ZSTD(1)), MetricName String CODEC(ZSTD(1)), MetricDescription String CODEC(ZSTD(1)), MetricUnit String CODEC(ZSTD(1)),
 Attributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 StartTimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)), TimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)),
 Value Float64 CODEC(ZSTD(1)), Flags UInt32 CODEC(ZSTD(1)),
 Exemplars Nested (FilteredAttributes Map(LowCardinality(String), String), TimeUnix DateTime64(9), Value Float64, SpanId String, TraceId String) CODEC(ZSTD(1)),
 AggregationTemporality Int32 CODEC(ZSTD(1)), IsMonotonic Boolean CODEC(Delta, ZSTD(1)),
 AgentId String MATERIALIZED Attributes['tilde.agent.id'],
 INDEX attr_key_idx mapKeys(Attributes) TYPE bloom_filter(0.01) GRANULARITY 1
) ENGINE = MergeTree
PARTITION BY toDate(TimeUnix)
ORDER BY (AgentId, MetricName, TimeUnix)
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS otel_metrics_gauge (
 ResourceAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)), ResourceSchemaUrl String CODEC(ZSTD(1)),
 ScopeName String CODEC(ZSTD(1)), ScopeVersion String CODEC(ZSTD(1)), ScopeAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 ScopeDroppedAttrCount UInt32 CODEC(ZSTD(1)), ScopeSchemaUrl String CODEC(ZSTD(1)),
 ServiceName LowCardinality(String) CODEC(ZSTD(1)), MetricName String CODEC(ZSTD(1)), MetricDescription String CODEC(ZSTD(1)), MetricUnit String CODEC(ZSTD(1)),
 Attributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 StartTimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)), TimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)),
 Value Float64 CODEC(ZSTD(1)), Flags UInt32 CODEC(ZSTD(1)),
 Exemplars Nested (FilteredAttributes Map(LowCardinality(String), String), TimeUnix DateTime64(9), Value Float64, SpanId String, TraceId String) CODEC(ZSTD(1)),
 AgentId String MATERIALIZED Attributes['tilde.agent.id'],
 INDEX attr_key_idx mapKeys(Attributes) TYPE bloom_filter(0.01) GRANULARITY 1
) ENGINE = MergeTree
PARTITION BY toDate(TimeUnix)
ORDER BY (AgentId, MetricName, TimeUnix)
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS otel_metrics_histogram (
 ResourceAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)), ResourceSchemaUrl String CODEC(ZSTD(1)),
 ScopeName String CODEC(ZSTD(1)), ScopeVersion String CODEC(ZSTD(1)), ScopeAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 ScopeDroppedAttrCount UInt32 CODEC(ZSTD(1)), ScopeSchemaUrl String CODEC(ZSTD(1)),
 ServiceName LowCardinality(String) CODEC(ZSTD(1)), MetricName String CODEC(ZSTD(1)), MetricDescription String CODEC(ZSTD(1)), MetricUnit String CODEC(ZSTD(1)),
 Attributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 StartTimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)), TimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)),
 Count UInt64 CODEC(Delta, ZSTD(1)), Sum Float64 CODEC(ZSTD(1)), BucketCounts Array(UInt64) CODEC(ZSTD(1)), ExplicitBounds Array(Float64) CODEC(ZSTD(1)),
 Exemplars Nested (FilteredAttributes Map(LowCardinality(String), String), TimeUnix DateTime64(9), Value Float64, SpanId String, TraceId String) CODEC(ZSTD(1)),
 Flags UInt32 CODEC(ZSTD(1)), Min Float64 CODEC(ZSTD(1)), Max Float64 CODEC(ZSTD(1)), AggregationTemporality Int32 CODEC(ZSTD(1)),
 AgentId String MATERIALIZED Attributes['tilde.agent.id'],
 INDEX attr_key_idx mapKeys(Attributes) TYPE bloom_filter(0.01) GRANULARITY 1
) ENGINE = MergeTree
PARTITION BY toDate(TimeUnix)
ORDER BY (AgentId, MetricName, TimeUnix)
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS otel_metrics_exponential_histogram (
 ResourceAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)), ResourceSchemaUrl String CODEC(ZSTD(1)),
 ScopeName String CODEC(ZSTD(1)), ScopeVersion String CODEC(ZSTD(1)), ScopeAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 ScopeDroppedAttrCount UInt32 CODEC(ZSTD(1)), ScopeSchemaUrl String CODEC(ZSTD(1)),
 ServiceName LowCardinality(String) CODEC(ZSTD(1)), MetricName String CODEC(ZSTD(1)), MetricDescription String CODEC(ZSTD(1)), MetricUnit String CODEC(ZSTD(1)),
 Attributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 StartTimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)), TimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)),
 Count UInt64 CODEC(Delta, ZSTD(1)), Sum Float64 CODEC(ZSTD(1)), Scale Int32 CODEC(ZSTD(1)), ZeroCount UInt64 CODEC(ZSTD(1)),
 PositiveOffset Int32 CODEC(ZSTD(1)), PositiveBucketCounts Array(UInt64) CODEC(ZSTD(1)), NegativeOffset Int32 CODEC(ZSTD(1)), NegativeBucketCounts Array(UInt64) CODEC(ZSTD(1)),
 Exemplars Nested (FilteredAttributes Map(LowCardinality(String), String), TimeUnix DateTime64(9), Value Float64, SpanId String, TraceId String) CODEC(ZSTD(1)),
 Flags UInt32 CODEC(ZSTD(1)), Min Float64 CODEC(ZSTD(1)), Max Float64 CODEC(ZSTD(1)), AggregationTemporality Int32 CODEC(ZSTD(1)),
 AgentId String MATERIALIZED Attributes['tilde.agent.id'],
 INDEX attr_key_idx mapKeys(Attributes) TYPE bloom_filter(0.01) GRANULARITY 1
) ENGINE = MergeTree
PARTITION BY toDate(TimeUnix)
ORDER BY (AgentId, MetricName, TimeUnix)
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS otel_metrics_summary (
 ResourceAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)), ResourceSchemaUrl String CODEC(ZSTD(1)),
 ScopeName String CODEC(ZSTD(1)), ScopeVersion String CODEC(ZSTD(1)), ScopeAttributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 ScopeDroppedAttrCount UInt32 CODEC(ZSTD(1)), ScopeSchemaUrl String CODEC(ZSTD(1)),
 ServiceName LowCardinality(String) CODEC(ZSTD(1)), MetricName String CODEC(ZSTD(1)), MetricDescription String CODEC(ZSTD(1)), MetricUnit String CODEC(ZSTD(1)),
 Attributes Map(LowCardinality(String), String) CODEC(ZSTD(1)),
 StartTimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)), TimeUnix DateTime64(9) CODEC(Delta, ZSTD(1)),
 Count UInt64 CODEC(Delta, ZSTD(1)), Sum Float64 CODEC(ZSTD(1)), ValueAtQuantiles Nested(Quantile Float64, Value Float64) CODEC(ZSTD(1)), Flags UInt32 CODEC(ZSTD(1)),
 AgentId String MATERIALIZED Attributes['tilde.agent.id'],
 INDEX attr_key_idx mapKeys(Attributes) TYPE bloom_filter(0.01) GRANULARITY 1
) ENGINE = MergeTree
PARTITION BY toDate(TimeUnix)
ORDER BY (AgentId, MetricName, TimeUnix)
SETTINGS index_granularity = 8192;

