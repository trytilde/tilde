--: Record()
--! run (agent_id, session_ids, identity_ids) : Record
SELECT p.thread_id AS session_id,p.user_id AS identity_id,COALESCE(i.value,n.value,u.name) AS value
FROM chat_participants p JOIN chat_users u ON u.id=p.user_id
LEFT JOIN chat_channel_threads b ON b.thread_id=p.thread_id
LEFT JOIN chat_channel_identities i ON i.id=u.id AND i.connection_id=b.connection_id
LEFT JOIN chat_native_identities n ON n.id=u.id
WHERE p.thread_id=ANY(:session_ids) AND p.user_id=ANY(:identity_ids)
AND (b.connection_id IS NULL OR i.id IS NOT NULL)
AND EXISTS(SELECT 1 FROM chat_participants owner WHERE owner.thread_id=p.thread_id AND owner.agent_id=:agent_id);
