--: Record()

--! run (p1, p2, p3) : Record
INSERT INTO thread_leases(thread_id,agent_id,instance_id) VALUES(:p1,:p2,:p3) ON CONFLICT(thread_id,agent_id) DO NOTHING RETURNING instance_id,updated_at;
