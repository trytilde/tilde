SELECT EXISTS(SELECT 1 FROM chat_invocations
 WHERE id=$1 AND agent_id=$2 AND thread_id=$3 AND run_id=$4
 AND ((status='running' AND lease_expires_at>NOW() AND to_timestamp($5)>NOW())
 OR (status IN ('stopped','failed','canceled') AND ended_at IS NOT NULL
 AND ended_at + INTERVAL '5 minutes'>NOW() AND ended_at<=to_timestamp($5)))) AS "live!";
