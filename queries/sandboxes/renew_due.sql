--: Record()

--! run : Record
-- Running, unleased sandboxes the provider would pause or end within twenty minutes.
SELECT id,connection_id,provider_sandbox_id FROM sandboxes
WHERE status='running' AND provider_sandbox_id IS NOT NULL AND expires_at < NOW() + INTERVAL '20 minutes'
  AND (lease_until IS NULL OR lease_until <= NOW())
LIMIT 50;
