--: Record(encrypted_secrets?, serving_deployment_id?)

--! run (p1) : Record
SELECT a.id,a.paused,a.generation,d.failure_mode,d.encrypted_secrets,d.routing,d.serving_deployment_id FROM agents a JOIN agent_deployment_settings d ON d.agent_id=a.id WHERE a.id=:p1 AND a.deleted_at IS NULL FOR UPDATE OF a,d;
