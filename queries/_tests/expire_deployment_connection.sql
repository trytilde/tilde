UPDATE agent_instances SET last_seen_at=NOW()-INTERVAL '1 minute' WHERE instance_id=$1;
