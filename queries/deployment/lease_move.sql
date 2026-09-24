--! run (p1, p2, p3)
UPDATE thread_leases SET instance_id=:p3,updated_at=clock_timestamp() WHERE thread_id=:p1 AND agent_id=:p2;
