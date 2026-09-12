SELECT COUNT(*) AS "count!",COALESCE(MAX(origin_sequence),0) AS "max_sequence!" FROM sidecar_archive WHERE agent_id=$1 AND thread_id=$2 AND origin_instance_id=$3;
