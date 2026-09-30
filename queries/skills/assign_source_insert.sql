--! run (agent, source_id)
INSERT INTO agent_skill_sources(agent_id,source_id) VALUES(:agent,:source_id) ON CONFLICT DO NOTHING;
