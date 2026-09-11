WITH eligible AS (
 SELECT i.id,a.generation FROM chat_invocations i JOIN agents a ON a.id=i.agent_id JOIN chat_runs r ON r.id=i.run_id
 WHERE i.id=$1 AND i.status='pending' AND NOT a.paused AND a.deleted_at IS NULL AND chat_channel_run_allowed(i.agent_id,i.thread_id,r.source_identity_id,r.channel_origin) FOR SHARE OF a
)
UPDATE chat_invocations i SET status='running',started_at=NOW(),lease_expires_at=NOW()+INTERVAL '30 seconds'
FROM eligible e WHERE i.id=e.id AND i.status='pending'
RETURNING i.id,i.run_id,i.thread_id,i.agent_id,i.traceparent,i.tracestate,e.generation AS "generation!";
