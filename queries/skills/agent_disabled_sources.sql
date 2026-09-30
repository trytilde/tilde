--: Record()

-- The agent's groups that are switched off.
--! run (agent) : Record
SELECT source_id FROM agent_skill_sources WHERE agent_id=:agent AND NOT enabled ORDER BY source_id;
