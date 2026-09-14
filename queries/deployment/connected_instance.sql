SELECT instance_id FROM agent_instances WHERE agent_id=$1 AND deployment_id=$2 AND ready AND agent_ready AND last_seen_at>NOW()-INTERVAL '15 seconds' ORDER BY last_seen_at DESC LIMIT 1;
