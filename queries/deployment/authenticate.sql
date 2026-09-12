SELECT a.id FROM agents a JOIN agent_deployments d ON d.agent_id=a.id WHERE d.token_hash=$1 AND a.deployment_mode='sidecar' AND a.deleted_at IS NULL;
