--! run (p1)
-- A call that answered "pending" goes on past its invocation, as a background call does.
UPDATE chat_tool_calls SET detached = TRUE, updated_at = NOW() WHERE id = :p1 AND status = 'running';
