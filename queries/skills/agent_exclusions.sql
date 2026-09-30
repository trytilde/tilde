--: Record()

-- The skills of the agent's groups that it switched off.
--! run (agent) : Record
SELECT skill_id FROM agent_skill_exclusions WHERE agent_id=:agent ORDER BY skill_id;
