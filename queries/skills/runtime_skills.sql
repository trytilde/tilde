--: Record()

-- What an invocation is given, each skill once at its latest version: every skill of its
-- enabled sources but those switched off, its single skills, and the sources linked to ready
-- connections whose skills capability it holds.
--! run (agent) : Record
SELECT DISTINCT ON (s.id) s.id,s.name,src.slug AS source_slug,v.id AS version_id,v.description
FROM skills s JOIN skill_sources src ON src.id=s.source_id
JOIN LATERAL (SELECT id,description FROM skill_versions v WHERE v.skill_id=s.id ORDER BY v.number DESC LIMIT 1) v ON true
WHERE (EXISTS(SELECT 1 FROM agent_skill_sources a WHERE a.agent_id=:agent AND a.source_id=s.source_id AND a.enabled)
  AND NOT EXISTS(SELECT 1 FROM agent_skill_exclusions x WHERE x.agent_id=:agent AND x.skill_id=s.id))
 OR EXISTS(SELECT 1 FROM agent_skills a WHERE a.agent_id=:agent AND a.skill_id=s.id)
 OR EXISTS(SELECT 1 FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id AND c.status='ready'
  WHERE ca.agent_id=:agent AND ca.capability='skills'
  AND EXISTS(SELECT 1 FROM connection_skill_sources l WHERE l.connection_id=ca.connection_id AND l.source_id=s.source_id))
ORDER BY s.id;
