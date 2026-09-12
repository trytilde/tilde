SELECT instance_id FROM sidecar_nodes WHERE agent_id=$1 AND instance_id<>$2 AND ready AND agent_ready AND last_seen_at>NOW()-INTERVAL '15 seconds' ORDER BY instance_id LIMIT 1;
