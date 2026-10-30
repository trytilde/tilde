--! run (p1, p2, p3)
-- Release bank p1 after a failure, due again in p2 seconds.
UPDATE memory_synthesis SET lease_until='-infinity',run_after=NOW()+make_interval(secs=>:p2::DOUBLE PRECISION),
  attempts=attempts+1,last_error=left(:p3,2000),last_run_at=NOW()
WHERE bank_id=:p1;
