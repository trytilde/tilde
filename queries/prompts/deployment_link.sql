--! run (deployment_id, version_id)
INSERT INTO deployment_prompts(deployment_id,version_id) VALUES(:deployment_id,:version_id) ON CONFLICT DO NOTHING;
