--: Record()

--! run (p1?, p2?, p3?, p4?) : Record
-- The agent's (p1) or sandbox blueprint's (p4) source for a connection (p2) or tool host (p3).
SELECT id FROM agent_tool_sources WHERE (agent_id=:p1 OR sandbox_blueprint_id=:p4) AND (connection_id=:p2 OR tool_host_id=:p3);
