--! run (agent, skill_id)
DELETE FROM agent_skills WHERE agent_id=:agent AND skill_id=:skill_id;
