UPDATE sidecar_nodes SET ready=$3,agent_ready=$4,last_seen_at=NOW() WHERE agent_id=$1 AND instance_id=$2;
