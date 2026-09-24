--: Record(icon_url?, agent_identity_type?, agent_identity_value?)

--! run (p1, p2?, p3) : Record
SELECT ca.connection_id,ca.agent_id,ca.access_mode,c.name,c.provider_id,c.type_id,c.status,p.name AS provider_name,p.icon_url,a.name AS agent_name, ai.identity_type AS agent_identity_type, ai.value AS agent_identity_value
FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id JOIN connection_providers p ON p.provider_id=c.provider_id
JOIN agents a ON a.id=ca.agent_id AND a.deleted_at IS NULL
LEFT JOIN agent_channel_identities ai ON ai.connection_id=ca.connection_id AND ai.agent_id=ca.agent_id
WHERE ca.agent_id=:p1 AND ca.capability='channel' AND (:p2::UUID IS NULL OR ca.connection_id>:p2) ORDER BY ca.connection_id LIMIT :p3;
