--: Record(source_id?)

--! run (p1) : Record
-- The agent's sandbox setting with the blueprint it launches from and its sandbox tool source.
SELECT a.agent_id,a.blueprint_id,b.reuse,b.connection_id,b.template,s.id AS source_id
FROM agent_sandboxes a JOIN sandbox_blueprints b ON b.id=a.blueprint_id
LEFT JOIN agent_tool_sources s ON s.sandbox_agent_id=a.agent_id
WHERE a.agent_id=:p1;
