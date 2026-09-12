INSERT INTO chat_runs(id,thread_id,agent_id,objective,status,goal_id,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status;
