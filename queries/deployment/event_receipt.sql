INSERT INTO sidecar_events(event_id,agent_id,thread_id,origin_instance_id,origin_sequence,created_at) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(event_id) DO NOTHING RETURNING event_id;
