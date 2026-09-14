UPDATE agent_deployments SET token_hash=$2 WHERE id=$1 AND agent_id=$3 AND status='registered' RETURNING id;
