--! run (p1, p2, p3)
DELETE FROM thread_leases WHERE thread_id=:p1 AND agent_id=:p2 AND instance_id=:p3;
