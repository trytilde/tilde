UPDATE sidecar_directives SET acked_at=NOW(),result=$4 WHERE id=$1 AND agent_id=$2 AND instance_id=$3 AND acked_at IS NULL RETURNING id;
