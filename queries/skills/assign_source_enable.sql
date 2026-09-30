--: Record()

-- Switching a group on turns all of its skills on; switching it off turns them all off, single
-- skills given from it included. Zero when the agent does not have the group.
--! run (agent, source_id, enabled) : Record
WITH switched AS (
 UPDATE agent_skill_sources SET enabled=:enabled WHERE agent_id=:agent AND source_id=:source_id RETURNING source_id
), cleared AS (
 DELETE FROM agent_skill_exclusions x USING skills s, switched
 WHERE x.agent_id=:agent AND s.id=x.skill_id AND s.source_id=switched.source_id
), singles AS (
 DELETE FROM agent_skills a USING skills s, switched
 WHERE NOT :enabled AND a.agent_id=:agent AND s.id=a.skill_id AND s.source_id=switched.source_id
)
SELECT COUNT(*)::INTEGER AS switched FROM switched;
