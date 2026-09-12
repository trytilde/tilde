SELECT MIN(deadline) AS deadline FROM (
 SELECT CASE WHEN c.acked_at IS NULL THEN c.dispatched_at+INTERVAL '5 seconds' ELSE n.last_seen_at+INTERVAL '15 seconds' END AS deadline
 FROM sidecar_commands c JOIN participant_assignments p ON p.thread_id=c.thread_id AND p.participant_id=c.participant_id AND p.generation=c.generation AND p.owner_instance_id=c.owner_instance_id
 JOIN agents a ON a.id=c.agent_id LEFT JOIN sidecar_nodes n ON n.agent_id=c.agent_id AND n.instance_id=c.owner_instance_id
 WHERE NOT EXISTS(SELECT 1 FROM chat_invocations old WHERE old.id=c.invocation_id AND old.status IN ('stopped','canceled')) AND NOT p.stopped AND NOT a.paused AND a.deleted_at IS NULL AND c.dispatched_at IS NOT NULL AND (c.acked_at IS NULL OR c.finished_at IS NULL)
) deadlines WHERE deadline>NOW();
