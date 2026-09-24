--! run (p1, p2)
UPDATE agent_deployment_settings SET encrypted_secrets=:p2 WHERE agent_id=:p1;
