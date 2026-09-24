-- HyperDX source mapping: Timestamp, Body, SeverityText, ServiceName.
-- Tilde adds its mandatory agent scope, FINAL deduplication and stable cursor.
SELECT LogId AS id, toString(toUnixTimestamp64Nano(Timestamp)) AS timestamp_ns,
 SeverityText AS severity, SeverityNumber AS severity_number, Body AS body,
 ServiceName AS service, TraceId AS trace_id, SpanId AS span_id,
 InvocationId AS invocation_id, ThreadId AS thread_id, LogAttributes['tilde.run.id'] AS run_id,
 LogAttributes AS attributes, ResourceAttributes AS resource_attributes,
 ScopeAttributes AS scope_attributes, ScopeName AS scope_name,
 if(InvocationId = '', 'deployment', 'invocation') AS scope, LogAttributes['tilde.deployment.id'] AS deployment_id
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
 AND ({cursor_id:String} = '' OR (Timestamp, LogId) < (fromUnixTimestamp64Nano({cursor_time:Int64}), {cursor_id:String}))
ORDER BY Timestamp DESC, LogId DESC LIMIT 101
SETTINGS max_execution_time = 5, max_result_bytes = 8388608, result_overflow_mode = 'throw', max_memory_usage = 268435456
FORMAT JSON
