--! run (p1)
DELETE FROM tool_host_health WHERE checked_at < :p1::TIMESTAMPTZ - INTERVAL '14 days';
