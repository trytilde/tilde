--: Record()

--! run (p1, p2) : Record
SELECT thread_id,updated_at FROM thread_leases WHERE agent_id=:p1 AND instance_id=:p2 ORDER BY thread_id;
