-- Whole-trace context, read only after membership.sql proved the caller's agent is part of it.
SELECT SpanId AS id, TraceId AS trace_id, ParentSpanId AS parent_id, SpanName AS name,
 ObservationType AS type, Level AS level, StatusMessage AS status_message,
 toString(toUnixTimestamp64Nano(Timestamp)) AS start_ns, toString(Duration) AS duration_ns,
 substringUTF8(SpanAttributes['tilde.observation.input'], 1, 200000) AS input,
 substringUTF8(SpanAttributes['tilde.observation.output'], 1, 200000) AS output,
 Model AS model, SpanAttributes['tilde.observation.completion_start_time'] AS completion_start_time,
 SpanAttributes['gen_ai.usage.input_tokens'] AS input_tokens, SpanAttributes['gen_ai.usage.output_tokens'] AS output_tokens,
 SpanAttributes['tilde.observation.cost_usd'] AS cost_usd,
 AgentId AS agent_id, ThreadId AS session_id, InvocationId AS invocation_id,
 mapFilter((k, v) -> k NOT IN ('tilde.observation.input', 'tilde.observation.output'), SpanAttributes) AS attributes,
 ResourceAttributes AS resource_attributes
FROM otel_traces FINAL
WHERE TraceId = {trace:String}
 AND Timestamp >= fromUnixTimestamp64Nano({from:Int64})
 AND Timestamp < fromUnixTimestamp64Nano({to:Int64})
 AND ({cursor_span:String} = '' OR (Timestamp, TraceId, SpanId) < (fromUnixTimestamp64Nano({cursor_time:Int64}), {cursor_trace:String}, {cursor_span:String}))
ORDER BY Timestamp DESC, TraceId DESC, SpanId DESC LIMIT 51
SETTINGS max_execution_time = 10, max_result_bytes = 33554432, result_overflow_mode = 'throw', max_memory_usage = 536870912
FORMAT JSON
