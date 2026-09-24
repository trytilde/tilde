--! run (p1, p2, p3)
-- A closing old stream cannot disconnect its replacement.
UPDATE agent_instances SET connection_id=NULL,ready=false,agent_connected=false WHERE agent_id=:p1 AND instance_id=:p2 AND connection_id=:p3;
