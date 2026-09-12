INSERT INTO sidecar_commands(id,generation,agent_id,thread_id,participant_id,owner_instance_id,kind,payload,invocation_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING;
