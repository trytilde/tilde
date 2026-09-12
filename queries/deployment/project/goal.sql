INSERT INTO chat_goals(id,thread_id,agent_id,objective,status) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status;
