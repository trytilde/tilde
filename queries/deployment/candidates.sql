SELECT a.id FROM agents a WHERE a.deployment_mode='sidecar' AND a.deleted_at IS NULL;
