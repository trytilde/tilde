-- Switch one skill of an enabled group off (excluded, and no longer given on its own either) or
-- back on for an agent.
--! run (agent, skill_id, excluded)
WITH restored AS (
 DELETE FROM agent_skill_exclusions WHERE agent_id=:agent AND skill_id=:skill_id AND NOT :excluded
), single AS (
 DELETE FROM agent_skills WHERE agent_id=:agent AND skill_id=:skill_id AND :excluded
)
INSERT INTO agent_skill_exclusions(agent_id,skill_id) SELECT :agent, :skill_id WHERE :excluded
ON CONFLICT DO NOTHING;
