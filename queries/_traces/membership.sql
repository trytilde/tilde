-- A trace is opened only after proving one of its spans belongs to the agent. The bounds then
-- confine the unscoped read of the whole trace, which includes gateway and cooperating-agent spans.
SELECT count() AS spans, toString(toUnixTimestamp64Nano(min(Timestamp))) AS first_ns,
 toString(toUnixTimestamp64Nano(max(Timestamp))) AS last_ns
FROM otel_traces
WHERE AgentId = {agent:String} AND TraceId = {trace:String}
SETTINGS max_execution_time = 10
FORMAT JSON
