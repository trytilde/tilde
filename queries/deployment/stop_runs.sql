UPDATE chat_runs SET status='failed' WHERE thread_id=$1 AND agent_id=$2 AND status IN ('active','suspending') RETURNING id;
