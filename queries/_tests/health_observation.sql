INSERT INTO agent_health(agent_id,checked_at,healthy,degraded,latency_ms,error_code)
VALUES($1,$2,$3,$4,0,CASE WHEN $3 THEN NULL ELSE 'not_ready' END);
