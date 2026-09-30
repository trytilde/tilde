--: Record()
-- The agents given each whole group: directly, or through a ready
-- connection whose skills capability links it.
--! run (source_ids) : Record
SELECT DISTINCT g.source_id, g.agent_id
FROM (
 SELECT a.source_id, a.agent_id FROM agent_skill_sources a WHERE a.source_id=ANY(:source_ids::UUID[]) AND a.enabled
 UNION SELECT l.source_id, ca.agent_id FROM connection_skill_sources l
  JOIN connections c ON c.id=l.connection_id AND c.status='ready'
  JOIN connection_agents ca ON ca.connection_id=l.connection_id AND ca.capability='skills'
  WHERE l.source_id=ANY(:source_ids::UUID[])
) g
JOIN agents ag ON ag.id=g.agent_id AND ag.deleted_at IS NULL
ORDER BY 1, 2;
