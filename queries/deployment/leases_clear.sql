--! run (p1)
DELETE FROM thread_leases WHERE agent_id=:p1;
