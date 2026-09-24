--! run (p1, p2, p3)
UPDATE agent_deployment_settings SET failure_mode=:p2,routing=:p3 WHERE agent_id=:p1;
