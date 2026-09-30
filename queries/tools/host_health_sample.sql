--! run (p1, p2)
-- p1: sample time; p2: liveness window in seconds.
INSERT INTO tool_host_health(tool_host_id,checked_at,healthy)
SELECT id,:p1,(execution_type='lambda' OR COALESCE(connected_at > :p1::TIMESTAMPTZ - make_interval(secs => :p2), false))
FROM tool_hosts;
