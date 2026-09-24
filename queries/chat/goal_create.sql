--! run (p1, p2, p3, p4)
INSERT INTO chat_goals(id,thread_id,agent_id,objective) VALUES(:p1,:p2,:p3,:p4) ON CONFLICT(id) DO NOTHING;
