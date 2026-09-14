INSERT INTO thread_leases(thread_id,agent_id,instance_id) VALUES($1,$2,$3) ON CONFLICT(thread_id,agent_id) DO NOTHING RETURNING instance_id,updated_at;
