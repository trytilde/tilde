--: Record()

--! run (p1, p2?, p3?) : Record
SELECT id FROM agent_tool_sources WHERE agent_id=:p1 AND (connection_id=:p2 OR tool_host_id=:p3);
