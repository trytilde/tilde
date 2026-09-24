--! run (p1, p2, p3)
UPDATE agent_instances SET connection_id=:p3,ready=false,agent_ready=false,agent_connected=false,last_seen_at=NOW() WHERE agent_id=:p1 AND instance_id=:p2;
