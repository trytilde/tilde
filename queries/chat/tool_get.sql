--: Record(personal_connection_id?)

--! run (p1) : Record
SELECT * FROM chat_tool_calls WHERE id=:p1 FOR UPDATE;
