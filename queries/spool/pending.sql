--: Record()
--! run (queue) : Record
SELECT COUNT(*)::BIGINT AS objects FROM telemetry_objects WHERE queue=:queue;
