SELECT thread_id,updated_at FROM thread_leases WHERE agent_id=$1 AND instance_id=$2 ORDER BY thread_id;
