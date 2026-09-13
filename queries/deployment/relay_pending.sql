SELECT m.id AS message_id,m.thread_id,p.agent_id AS "agent_id!" FROM chat_messages m
JOIN chat_participants p ON p.thread_id=m.thread_id AND p.agent_id IS NOT NULL AND p.active
JOIN agents a ON a.id=p.agent_id AND a.deployment_mode='sidecar' AND NOT a.paused AND a.deleted_at IS NULL
JOIN chat_participants author ON author.id=m.participant_id
WHERE m.status='complete' AND author.agent_id IS DISTINCT FROM p.agent_id
AND NOT EXISTS(SELECT 1 FROM chat_message_dispatch d WHERE d.message_id=m.id AND d.agent_id=p.agent_id)
AND EXISTS(SELECT 1 FROM sidecar_nodes n WHERE n.agent_id=p.agent_id AND n.ready AND n.agent_ready AND n.last_seen_at>NOW()-INTERVAL '15 seconds')
ORDER BY m.created_at LIMIT 50;
