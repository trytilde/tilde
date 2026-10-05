--: Record(agent_id?, connection_id?, tool_host_id?, sandbox_blueprint_id?)

--! run (p1?, p2?, p3?, p4?, p5?) : Record
-- One source (p1), an agent's (p2), a connection's (p3), a tool host's (p4) or a sandbox
-- blueprint's (p5). `sandbox`: the agent's sandbox.
SELECT id,agent_id,connection_id,tool_host_id,slug,sandbox_blueprint_id,sandbox_agent_id IS NOT NULL AS sandbox FROM agent_tool_sources
WHERE (:p1::UUID IS NULL OR id=:p1)
  AND (:p2::UUID IS NULL OR agent_id=:p2)
  AND (:p3::UUID IS NULL OR connection_id=:p3)
  AND (:p4::UUID IS NULL OR tool_host_id=:p4)
  AND (:p5::UUID IS NULL OR sandbox_blueprint_id=:p5)
ORDER BY agent_id,sandbox_blueprint_id,slug;
