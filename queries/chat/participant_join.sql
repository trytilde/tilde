INSERT INTO chat_participants(id,thread_id,user_id,agent_id) VALUES($1,$2,$3,$4)
ON CONFLICT DO NOTHING;
