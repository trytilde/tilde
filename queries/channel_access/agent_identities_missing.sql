SELECT ca.connection_id,ca.agent_id FROM connection_agents ca
JOIN connections c ON c.id=ca.connection_id
JOIN agents a ON a.id=ca.agent_id AND a.deleted_at IS NULL
LEFT JOIN agent_channel_identities ai ON ai.connection_id=ca.connection_id AND ai.agent_id=ca.agent_id
WHERE ca.capability='channel' AND c.status='ready' AND ai.connection_id IS NULL;
