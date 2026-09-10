SELECT id,objective,goal_id FROM chat_runs WHERE thread_id=$1 AND agent_id=$2 AND idempotency_key=$3;
