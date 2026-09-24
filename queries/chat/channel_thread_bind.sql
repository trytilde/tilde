--! run (p1, p2, p3, p4)
INSERT INTO chat_channel_threads(thread_id,connection_id,external_id,agent_id) VALUES(:p1,:p2,:p3,:p4) ON CONFLICT DO NOTHING;
