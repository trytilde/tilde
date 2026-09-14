SELECT EXISTS(SELECT 1 FROM chat_invocations i JOIN agents a ON a.id=i.agent_id JOIN chat_runs r ON r.id=i.run_id
WHERE i.id=$1 AND i.agent_id=$2 AND i.thread_id=$3 AND i.run_id=$4 AND i.status='running'
AND i.lease_expires_at>NOW() AND chat_channel_run_allowed(i.agent_id,i.thread_id,r.source_identity_id,r.channel_origin) AND NOT a.paused AND a.deleted_at IS NULL) AS "live!";
