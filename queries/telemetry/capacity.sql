SELECT COALESCE(SUM(octet_length(payload)),0)::bigint AS "bytes!" FROM telemetry_delivery WHERE payload IS NOT NULL;
