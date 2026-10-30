--! run (p1, p2, p3)
-- Release bank p1, due again in p2 seconds; p3 when it ran a batch.
UPDATE memory_synthesis SET lease_until='-infinity',run_after=NOW()+make_interval(secs=>:p2::DOUBLE PRECISION),
  attempts=0,last_error='',last_run_at=CASE WHEN :p3::BOOLEAN THEN NOW() ELSE last_run_at END
WHERE bank_id=:p1;
