SELECT EXISTS(SELECT 1 FROM chat_invocations WHERE id=$1 AND agent_id=$2 AND thread_id=$3 AND run_id=$4 AND status='running' AND lease_expires_at>NOW()) AS "live!";
