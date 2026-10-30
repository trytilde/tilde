--! run (p1, p2, p3, p4?)
-- Inserts nothing when the name is taken.
INSERT INTO wikis(id,name,description,agent_id) VALUES(:p1,:p2,:p3,:p4)
ON CONFLICT(name) DO NOTHING;
