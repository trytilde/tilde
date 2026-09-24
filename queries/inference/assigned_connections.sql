--: Record()

--! run (p1) : Record
SELECT connection_id FROM connection_agents WHERE agent_id=:p1 AND capability='inference';
