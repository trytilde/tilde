SELECT thread_id,participant_id,owner_instance_id,generation,stopped,updated_at FROM participant_assignments WHERE agent_id=$1 AND updated_at>$2 ORDER BY updated_at,thread_id;
