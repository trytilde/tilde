-- Batches nobody delivered within the retention period, in every queue. A leased batch is
-- mid-delivery and completes or returns on its own.
--! run (retention_seconds)
DELETE FROM telemetry_objects
WHERE created_at<NOW()-make_interval(secs=>:retention_seconds::INTEGER) AND leased_until<=NOW();
