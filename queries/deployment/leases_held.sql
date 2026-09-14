SELECT thread_id FROM thread_leases WHERE agent_id=$1 AND instance_id=$2 ORDER BY thread_id;
