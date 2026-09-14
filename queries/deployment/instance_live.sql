SELECT EXISTS(SELECT 1 FROM agent_instances WHERE agent_id=$1 AND instance_id=$2 AND ready AND last_seen_at>NOW()-make_interval(secs=>$3::float8)) AS "live!";
