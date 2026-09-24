-- Adapted from HyperDX DBSearchPage histogram and renderChartConfig time bucketing.
-- https://github.com/hyperdxio/hyperdx/tree/c98be91f618fca6931ae0aea75553ea8b6573551/packages/common-utils/src/core
-- Keep the same exact [from,to) predicates as list.sql; do not expand to bucket edges.
SELECT toString(toUnixTimestamp64Nano(toDateTime64(
 toStartOfInterval(Timestamp, toIntervalSecond({bucket_seconds:UInt32}), 'UTC'), 9, 'UTC'))) AS bucket_ns,
 SeverityText AS severity, SeverityNumber AS severity_number, count() AS log_count
FROM otel_logs FINAL
WHERE AgentId = {agent:String}
 AND Timestamp >= fromUnixTimestamp64Nano({from:Int64})
 AND Timestamp < fromUnixTimestamp64Nano({to:Int64})
 AND SeverityNumber >= {severity:Int32}
 AND ({search:String} = '' OR Body ILIKE {search_pattern:String})
 AND ({invocation:String} = '' OR InvocationId = {invocation:String})
 AND ({trace:String} = '' OR TraceId = {trace:String})
 AND ({thread:String} = '' OR ThreadId = {thread:String})
 AND ({run:String} = '' OR LogAttributes['tilde.run.id'] = {run:String})
 AND ({scope:String} = '' OR if(InvocationId = '', 'deployment', 'invocation') = {scope:String})
 AND ({service:String} = '' OR ServiceName = {service:String})

GROUP BY bucket_ns, severity, severity_number ORDER BY bucket_ns
SETTINGS max_execution_time = 5, max_result_bytes = 8388608, result_overflow_mode = 'throw', max_memory_usage = 268435456
FORMAT JSON
