UPDATE thread_leases SET instance_id=$3,updated_at=clock_timestamp() WHERE thread_id=$1 AND agent_id=$2;
