SELECT id,payload AS "payload!",attempts FROM telemetry_delivery
WHERE payload IS NOT NULL AND retry_at<=NOW() ORDER BY retry_at LIMIT 1 FOR UPDATE SKIP LOCKED;
