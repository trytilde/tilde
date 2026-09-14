SELECT ca.connection_id,ca.agent_id,ca.access_mode,c.name,c.provider_id,c.type_id,c.status,p.name AS provider_name,p.icon_url, a.name AS agent_name
FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id JOIN connection_providers p ON p.provider_id=c.provider_id JOIN agents a ON a.id=ca.agent_id
WHERE ca.connection_id=$1 AND ca.agent_id=$2 AND ca.capability='channel' AND a.deleted_at IS NULL;
