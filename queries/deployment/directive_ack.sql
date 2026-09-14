UPDATE sidecar_directives SET acked_at=NOW() WHERE id=$1 AND agent_id=$2 AND acked_at IS NULL RETURNING id;
