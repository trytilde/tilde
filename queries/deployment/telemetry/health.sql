--! run (p1, p2, p3, p4, p5, p6)
INSERT INTO agent_health(sidecar_event_id,agent_id,instance_id,checked_at,healthy,latency_ms,error_code) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,CASE WHEN :p5 THEN NULL ELSE 'not_ready' END) ON CONFLICT(sidecar_event_id) DO NOTHING;
