--: Record()

--! run (p1) : Record
SELECT EXISTS(SELECT 1 FROM telemetry_delivery WHERE id=:p1) AS exists;
