UPDATE chat_runs SET status='active' WHERE id=$1 AND status IN ('waiting','failed') AND chat_channel_run_allowed(agent_id,thread_id,source_identity_id,channel_origin) RETURNING thread_id,agent_id;
