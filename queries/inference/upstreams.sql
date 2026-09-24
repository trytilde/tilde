--: Record()

--! run : Record
SELECT c.id,c.provider_id,c.type_id,c.name,ARRAY(SELECT ca.agent_id FROM connection_agents ca WHERE ca.connection_id=c.id AND ca.capability='inference' ORDER BY ca.agent_id) AS agents FROM connections c JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id) WHERE t.inference_capable AND c.status='ready' ORDER BY c.created_at;
