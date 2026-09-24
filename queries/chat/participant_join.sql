--! run (p1, p2, p3?, p4?)
INSERT INTO chat_participants(id,thread_id,user_id,agent_id) VALUES(:p1,:p2,:p3,:p4)
ON CONFLICT DO NOTHING;
