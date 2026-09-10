INSERT INTO chat_runs(id,thread_id,agent_id,objective,goal_id,idempotency_key) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(thread_id,agent_id,idempotency_key) DO NOTHING RETURNING id;
