--! run (p1, p2, p3)
INSERT INTO chat_threads(id,title,primary_agent_id) VALUES(:p1,:p2,:p3) ON CONFLICT(id) DO UPDATE SET title=EXCLUDED.title;
