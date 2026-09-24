--! run (p1, p2, p3, p4, p5)
UPDATE agent_instances SET ready=:p3,agent_ready=:p4,agent_connected=:p5,last_seen_at=NOW() WHERE agent_id=:p1 AND instance_id=:p2 AND connection_id IS NOT NULL;
