SELECT id FROM chat_invocations WHERE thread_id=$1 AND agent_id=$2 AND status IN ('pending','running');
