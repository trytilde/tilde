SELECT COUNT(*) AS "count!" FROM sidecar_archive WHERE agent_id=$1 AND event_id=ANY($2);
