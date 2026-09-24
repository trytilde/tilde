--: Record(connection_id?, last_turn_time?)
--! run (agent_ids, session_ids) : Record
WITH requested AS (SELECT DISTINCT * FROM UNNEST(:agent_ids::UUID[],:session_ids::UUID[]) AS r(agent_id,session_id))
SELECT r.agent_id,t.id AS session_id,statement_timestamp() AS captured_at,
 COALESCE(c.provider_id,'tilde') AS provider_id,
 COALESCE(cp.name,CASE WHEN b.connection_id IS NULL THEN 'Tilde' ELSE c.provider_id END) AS provider_name,
 COALESCE(cp.icon_url,'') AS provider_icon_url,b.connection_id,
 ARRAY(SELECT DISTINCT p.user_id FROM chat_participants p LEFT JOIN chat_channel_identities i ON i.id=p.user_id AND i.connection_id=b.connection_id
 WHERE p.thread_id=t.id AND p.user_id IS NOT NULL AND (b.connection_id IS NULL OR i.id IS NOT NULL) ORDER BY 1) AS identity_ids,
 m.message_count,GREATEST(m.last_message_time,inv.last_turn_time) AS last_turn_time
FROM requested r JOIN chat_threads t ON t.id=r.session_id
LEFT JOIN chat_channel_threads b ON b.thread_id=t.id
LEFT JOIN connections c ON c.id=b.connection_id
LEFT JOIN connection_providers cp ON cp.provider_id=c.provider_id
LEFT JOIN LATERAL (
 SELECT COUNT(*)::BIGINT AS message_count,MAX(created_at) AS last_message_time
 FROM chat_messages m WHERE m.thread_id=t.id AND m.status<>'deleted'
 AND (length(m.text)>0 OR EXISTS(SELECT 1 FROM chat_message_attachments a WHERE a.message_id=m.id))
) m ON TRUE
LEFT JOIN LATERAL (SELECT MAX(started_at) AS last_turn_time FROM chat_invocations i WHERE i.thread_id=t.id) inv ON TRUE
WHERE EXISTS(SELECT 1 FROM chat_participants p WHERE p.thread_id=t.id AND p.agent_id=r.agent_id);
