SELECT EXISTS(SELECT 1 FROM agent_instances WHERE agent_id=$1 AND instance_id=$2 AND last_seen_at>NOW()-INTERVAL '15 seconds') AS "live!";
