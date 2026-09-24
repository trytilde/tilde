--! run (p1, p2)
UPDATE telemetry_delivery SET attempts=LEAST(attempts+1,20),retry_at=NOW()+make_interval(secs=>:p2::integer) WHERE id=:p1;
