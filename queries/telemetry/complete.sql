--! run (p1)
UPDATE telemetry_delivery SET payload=NULL,completed_at=NOW() WHERE id=:p1;
