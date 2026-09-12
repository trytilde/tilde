UPDATE telemetry_delivery SET attempts=LEAST(attempts+1,20),retry_at=NOW()+make_interval(secs=>$2::integer) WHERE id=$1;
