SELECT id,thread_id,payload FROM sidecar_directives WHERE agent_id=$1 AND instance_id=$2 AND acked_at IS NULL ORDER BY created_at,id;
