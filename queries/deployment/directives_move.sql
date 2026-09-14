UPDATE sidecar_directives SET instance_id=$3 WHERE thread_id=$1 AND instance_id=$2 AND acked_at IS NULL;
