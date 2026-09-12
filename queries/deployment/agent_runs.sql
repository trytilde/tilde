SELECT id,objective,status,goal_id,idempotency_key FROM chat_runs WHERE thread_id=$1 AND agent_id=$2 AND status IN ('active','suspending','waiting') ORDER BY id;
