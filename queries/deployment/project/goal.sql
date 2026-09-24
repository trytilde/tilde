--! run (p1, p2, p3, p4, p5)
INSERT INTO chat_goals(id,thread_id,agent_id,objective,status) VALUES(:p1,:p2,:p3,:p4,:p5) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status;
