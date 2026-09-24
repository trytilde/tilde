--: Record()

--! run (p1, p2, p3?, p4, p5, p6) : Record
INSERT INTO sidecar_events(event_id,agent_id,thread_id,origin_instance_id,origin_sequence,created_at) VALUES(:p1,:p2,:p3,:p4,:p5,:p6) ON CONFLICT(event_id) DO NOTHING RETURNING event_id;
