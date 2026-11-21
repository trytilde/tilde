--: Record()

--! run (p1) : Record
-- The agent's aliased inference assignments.
SELECT a.alias,a.connection_id,c.provider_id||'/'||c.name AS slug FROM connection_agents a JOIN connections c ON c.id=a.connection_id
WHERE a.agent_id=:p1 AND a.capability='inference' AND a.alias IS NOT NULL ORDER BY a.alias;
