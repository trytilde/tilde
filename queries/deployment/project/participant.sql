--! run (p1, p2, p3?, p4?, p5)
INSERT INTO chat_participants(id,thread_id,user_id,agent_id,active) VALUES(:p1,:p2,:p3,:p4,:p5) ON CONFLICT(id) DO UPDATE SET active=EXCLUDED.active;
