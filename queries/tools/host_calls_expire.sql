--! run
DELETE FROM tool_host_calls WHERE created_at < NOW() - INTERVAL '1 hour';
