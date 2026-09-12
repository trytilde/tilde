SELECT i.id,i.run_id,i.thread_id,i.agent_id,p.id AS participant_id,a.capabilities AS "capabilities!: Json<Capabilities>",i.status,i.lease_expires_at
FROM chat_invocations i JOIN agents a ON a.id=i.agent_id JOIN chat_participants p ON p.thread_id=i.thread_id AND p.agent_id=i.agent_id
WHERE i.id=$1 AND i.agent_id=$2 AND p.active AND NOT a.paused AND a.deleted_at IS NULL;
