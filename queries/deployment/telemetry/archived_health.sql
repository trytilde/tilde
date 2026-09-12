SELECT sidecar_event_id AS "id!" FROM agent_health WHERE agent_id=$1 AND sidecar_event_id=ANY($2);
