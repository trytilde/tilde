--: Record()
--! run (agent_id, provider_id, name) : Record
SELECT c.id FROM connections c JOIN connection_agents a ON a.connection_id=c.id AND a.capability='channel'
WHERE a.agent_id=:agent_id AND c.provider_id=:provider_id AND c.name=:name AND c.status='ready';
