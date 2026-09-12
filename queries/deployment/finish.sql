UPDATE sidecar_commands SET finished_at=COALESCE(finished_at,NOW()),failure=$5 WHERE id=$1 AND generation=$2 AND agent_id=$3 AND owner_instance_id=$4;
