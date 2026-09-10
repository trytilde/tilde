UPDATE chat_invocations SET status=$2,ended_at=NOW() WHERE id=$1 AND status IN ('pending','running') RETURNING run_id,thread_id;
