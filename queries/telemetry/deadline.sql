SELECT MIN(retry_at) AS retry_at FROM telemetry_delivery WHERE payload IS NOT NULL;
