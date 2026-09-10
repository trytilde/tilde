INSERT INTO chat_goals(id,thread_id,agent_id,objective) VALUES($1,$2,$3,$4) ON CONFLICT(id) DO NOTHING;
