--! run (p1, p2)
-- Release bank p1, due again now, after passing over a batch: its failures and error stay.
UPDATE memory_synthesis SET lease_until='-infinity',run_after=NOW(),last_error=left(:p2,2000),last_run_at=NOW()
WHERE bank_id=:p1;
