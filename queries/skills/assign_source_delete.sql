-- Removing a group also drops the skills from it given one by one, and its switched-off skills.
--! run (agent, source_id)
WITH removed AS (DELETE FROM agent_skill_sources WHERE agent_id=:agent AND source_id=:source_id),
cleared AS (
 DELETE FROM agent_skill_exclusions x USING skills s
 WHERE x.agent_id=:agent AND s.id=x.skill_id AND s.source_id=:source_id
)
DELETE FROM agent_skills a USING skills s
WHERE a.agent_id=:agent AND s.id=a.skill_id AND s.source_id=:source_id;
