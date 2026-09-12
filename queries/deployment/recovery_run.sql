SELECT r.id AS run_id,r.objective FROM chat_runs r WHERE r.thread_id=$1 AND r.agent_id=$2 AND r.status IN ('active','suspending') ORDER BY r.id LIMIT 1;
