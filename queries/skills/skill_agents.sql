--: Record()

-- The agents given each skill: through its whole group, the skill
-- itself, or a ready connection whose skills capability links its group.
--! run (skill_ids) : Record
SELECT DISTINCT s.id AS skill_id, g.agent_id
FROM skills s
JOIN LATERAL (
 SELECT a.agent_id FROM agent_skill_sources a WHERE a.source_id=s.source_id AND a.enabled
  AND NOT EXISTS(SELECT 1 FROM agent_skill_exclusions x WHERE x.agent_id=a.agent_id AND x.skill_id=s.id)
 UNION SELECT a.agent_id FROM agent_skills a WHERE a.skill_id=s.id
 UNION SELECT ca.agent_id FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id AND c.status='ready'
  WHERE ca.capability='skills'
  AND EXISTS(SELECT 1 FROM connection_skill_sources l WHERE l.connection_id=ca.connection_id AND l.source_id=s.source_id)
) g ON true
JOIN agents ag ON ag.id=g.agent_id AND ag.deleted_at IS NULL
WHERE s.id=ANY(:skill_ids::UUID[])
ORDER BY 1, 2;
