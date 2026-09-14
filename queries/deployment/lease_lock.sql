SELECT instance_id,updated_at FROM thread_leases WHERE thread_id=$1 AND agent_id=$2 FOR UPDATE;
