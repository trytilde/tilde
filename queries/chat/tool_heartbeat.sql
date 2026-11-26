--! run (p1)
-- A pending call still running in this process.
UPDATE chat_tool_calls SET heartbeat_at = NOW() WHERE id = :p1 AND status = 'running';
