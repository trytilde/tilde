INSERT INTO chat_participants(id,thread_id,user_id,agent_id,active) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET active=EXCLUDED.active;
