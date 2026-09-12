SELECT a.id,d.encrypted_secrets FROM agents a JOIN agent_deployments d ON d.agent_id=a.id WHERE a.deployment_mode='sidecar' AND a.deleted_at IS NULL AND d.encrypted_secrets IS NOT NULL;
