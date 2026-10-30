--! run (p1, p2, p3, p4?, p5)
-- Inserts nothing when the name is taken.
INSERT INTO memory_banks(id,name,description,agent_id,scope) VALUES(:p1,:p2,:p3,:p4,:p5)
ON CONFLICT(name) DO NOTHING;
