--: Record()

--! run (p1?) : Record
-- Running sandboxes the provider would pause or end within twenty minutes: every unleased one,
-- or only p1 (which the caller has just leased, to check it still is). `busy`: it has operations
-- queued or in flight.
SELECT s.id,s.connection_id,s.provider_sandbox_id,
  EXISTS (SELECT 1 FROM sandbox_calls c WHERE c.sandbox_id=s.id) AS busy
FROM sandboxes s
WHERE s.status='running' AND s.provider_sandbox_id IS NOT NULL AND s.expires_at < NOW() + INTERVAL '20 minutes'
  AND CASE WHEN :p1::UUID IS NULL THEN s.lease_until IS NULL OR s.lease_until <= NOW() ELSE s.id=:p1 END
LIMIT 50;
