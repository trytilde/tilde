UPDATE chat_runs SET status='active' WHERE id=$1 AND status IN ('waiting','failed') RETURNING thread_id,agent_id;
