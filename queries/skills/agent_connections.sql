--: Record()

-- Connections whose skills capability the agent holds.
--! run (agent) : Record
SELECT c.id,c.name,c.provider_id,c.status FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id
WHERE ca.agent_id=:agent AND ca.capability='skills' ORDER BY c.name, c.id;
