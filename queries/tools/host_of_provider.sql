--: Record(function_arn?)

--! run (p1, p2) : Record
-- The tool host a provider belongs to, and whether it can take a call now (p2: liveness seconds).
SELECT h.id,h.function_arn,
  (h.execution_type='lambda' OR COALESCE(h.connected_at > NOW() - make_interval(secs => :p2), false)) AS available
FROM connection_providers p JOIN tool_hosts h ON h.id=p.tool_host_id WHERE p.provider_id=:p1;
