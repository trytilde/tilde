INSERT INTO agent_health(agent_id,instance_id,checked_at,healthy,latency_ms)
VALUES($1,NULL,NOW()-INTERVAL '15 days',TRUE,0),($1,NULL,NOW()-INTERVAL '13 days',TRUE,0);
