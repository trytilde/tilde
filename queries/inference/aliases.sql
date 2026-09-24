--: Record()

--! run : Record
SELECT a.agent_id,a.alias,a.connection_id FROM connection_agents a JOIN connections c ON c.id=a.connection_id WHERE a.capability='inference' AND a.alias IS NOT NULL AND c.status='ready' ORDER BY a.agent_id,a.alias;
