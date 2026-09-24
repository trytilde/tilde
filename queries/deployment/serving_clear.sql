--! run (p1)
UPDATE agent_deployment_settings SET serving_deployment_id=NULL WHERE agent_id=:p1;
