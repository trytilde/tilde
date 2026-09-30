--: Record()

-- The agent's bundled source, holding the skills its deployments ship.
--! run (agent_id) : Record
SELECT id FROM skill_sources WHERE agent_id=:agent_id AND kind='bundled';
