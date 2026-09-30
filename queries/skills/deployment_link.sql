--! run (deployment_id, version_id, origin)
INSERT INTO deployment_skills(deployment_id,version_id,origin) VALUES(:deployment_id,:version_id,:origin) ON CONFLICT DO NOTHING;
