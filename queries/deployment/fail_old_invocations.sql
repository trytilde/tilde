UPDATE chat_invocations SET status='failed',ended_at=NOW() WHERE thread_id=$1 AND agent_id=$2 AND status IN ('pending','running') RETURNING id;
