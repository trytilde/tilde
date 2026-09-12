SELECT id,agent_id,kind,payload FROM sidecar_outbox WHERE delivered_at IS NULL AND next_attempt_at<=NOW() ORDER BY (kind='assignment') DESC,created_at,id LIMIT 50;
