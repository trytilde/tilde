SELECT r.source_identity_id,r.channel_origin,r.status,r.thread_id,r.agent_id,a.concurrency_policy
FROM chat_runs r JOIN agents a ON a.id=r.agent_id WHERE r.id=$1;
