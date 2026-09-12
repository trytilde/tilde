SELECT EXISTS(SELECT 1 FROM sidecar_conversations WHERE agent_id=$1 AND storage<>'postgres') AS "retained!";
