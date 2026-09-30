--! run (agent, skill_id)
INSERT INTO agent_skills(agent_id,skill_id) VALUES(:agent,:skill_id) ON CONFLICT DO NOTHING;
