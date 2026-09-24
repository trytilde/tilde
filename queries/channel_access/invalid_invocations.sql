--: Record()

--! run (p1, p2) : Record
SELECT i.id FROM chat_invocations i JOIN chat_runs r ON r.id=i.run_id JOIN chat_channel_threads b ON b.thread_id=i.thread_id
WHERE b.connection_id=:p1 AND i.agent_id=:p2 AND i.status IN ('pending','running') AND NOT chat_channel_run_allowed(i.agent_id,i.thread_id,r.source_identity_id,r.channel_origin);
