--: Record()

--! run (p1, p2, p3) : Record
INSERT INTO prompts(id,agent_id,name) VALUES(:p1,:p2,:p3)
ON CONFLICT (agent_id,name) DO UPDATE SET name=excluded.name
RETURNING id;