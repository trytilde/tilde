--! run (call, connection)
-- Audit: the personal connection a tool call runs on.
UPDATE chat_tool_calls SET personal_connection_id=:connection WHERE id=:call;
