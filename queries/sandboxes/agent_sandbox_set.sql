--! run (p1, p2)
INSERT INTO agent_sandboxes(agent_id,blueprint_id) VALUES(:p1,:p2)
ON CONFLICT(agent_id) DO UPDATE SET blueprint_id=excluded.blueprint_id;
