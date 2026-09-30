--: Record(connection_id?, tool_host_id?)

--! run (p1?, p2?, p3?, p4?) : Record
-- One source (p1), an agent's (p2), a connection's (p3) or a tool host's (p4).
SELECT id,agent_id,connection_id,tool_host_id,slug FROM agent_tool_sources
WHERE (:p1::UUID IS NULL OR id=:p1)
  AND (:p2::UUID IS NULL OR agent_id=:p2)
  AND (:p3::UUID IS NULL OR connection_id=:p3)
  AND (:p4::UUID IS NULL OR tool_host_id=:p4)
ORDER BY agent_id,slug;
