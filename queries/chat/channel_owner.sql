--: Record()

--! run (p1) : Record
SELECT ca.agent_id,ca.access_mode,c.provider_id,c.type_id FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=:p1 AND ca.capability='channel' AND c.status='ready';
