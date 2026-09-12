UPDATE telemetry_delivery SET payload=NULL,completed_at=NOW() WHERE id=$1;
