SELECT MIN(next_attempt_at) AS deadline FROM sidecar_outbox WHERE delivered_at IS NULL;
