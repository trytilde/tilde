--! run (p1, p2, p3)
INSERT INTO agent_wikis(agent_id,wiki_id,access) VALUES(:p1,:p2,:p3)
ON CONFLICT(agent_id,wiki_id) DO UPDATE SET access=EXCLUDED.access;
