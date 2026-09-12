UPDATE sidecar_outbox SET attempts=LEAST(attempts+1,10),next_attempt_at=NOW()+make_interval(secs=>LEAST(60,POWER(2,LEAST(attempts+1,6)))::integer) WHERE id=$1;
