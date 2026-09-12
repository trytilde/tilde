UPDATE agent_deployments SET failure_mode=$2,retention_days=$3 WHERE agent_id=$1;
