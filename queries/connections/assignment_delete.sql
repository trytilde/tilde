--! run (p1, p2, p3)
DELETE FROM connection_agents WHERE connection_id=:p1 AND capability=:p2 AND agent_id=:p3;
