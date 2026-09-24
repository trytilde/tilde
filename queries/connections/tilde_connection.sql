--: Record()

-- The system-managed Tilde connection an agent owns; at most one per agent.
--! run (p1) : Record
SELECT c.id FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id
WHERE ca.agent_id=:p1 AND ca.capability='channel' AND c.provider_id='tilde';
