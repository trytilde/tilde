--: Record(personal_connection_id?, heartbeat_at?)

--! run (p1) : Record
SELECT * FROM chat_tool_calls WHERE id=:p1 FOR UPDATE;
