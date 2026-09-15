SELECT v.id,v.connection_id,v.agent_id,v.identity_id,v.token_hash,v.status,v.expires_at,i.value,i.identity_type,c.name AS account_name,p.name AS provider_name,p.icon_url,a.name AS agent_name, ai.identity_type AS "agent_identity_type?", ai.value AS "agent_identity_value?"
FROM chat_identity_verifications v JOIN chat_channel_identities i ON i.id=v.identity_id JOIN connections c ON c.id=v.connection_id
JOIN connection_providers p ON p.provider_id=c.provider_id JOIN agents a ON a.id=v.agent_id
LEFT JOIN agent_channel_identities ai ON ai.connection_id=v.connection_id AND ai.agent_id=v.agent_id WHERE v.id=$1 AND a.deleted_at IS NULL;
