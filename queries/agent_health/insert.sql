-- Discard checks whose agent was deleted or whose endpoint changed during the request.
INSERT INTO agent_health(agent_id, endpoint_url, checked_at, healthy, latency_ms, error_code)
SELECT id, endpoint_url, $3, $4, $5, $6 FROM agents WHERE id = $1 AND endpoint_url = $2;
