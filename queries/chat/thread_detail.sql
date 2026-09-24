--: Record(participant_id?, user_id?, agent_id?, active?, name?, connection_id?, external_id?, provider_id?)

--! run (p1) : Record
SELECT t.id, t.title, t.primary_agent_id,
       p.id AS participant_id, p.user_id, p.agent_id, p.active AS active,
       COALESCE(u.name, a.name) AS name,
       b.connection_id AS connection_id, b.external_id AS external_id,
       c.provider_id AS provider_id
FROM chat_threads t
LEFT JOIN chat_participants p ON p.thread_id = t.id
LEFT JOIN chat_users u ON u.id = p.user_id
LEFT JOIN agents a ON a.id = p.agent_id
LEFT JOIN chat_channel_threads b ON b.thread_id = t.id
LEFT JOIN connections c ON c.id = b.connection_id
WHERE t.id = :p1
ORDER BY p.id;
