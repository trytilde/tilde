SELECT last_activity_at,storage FROM sidecar_conversations WHERE thread_id=$1 AND agent_id=$2 FOR UPDATE;
