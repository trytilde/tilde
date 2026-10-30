--: Record()

--! run (p1) : Record
-- Claim the bank that has waited longest, leasing it for p1 seconds.
UPDATE memory_synthesis s SET lease_until=NOW()+make_interval(secs=>:p1::DOUBLE PRECISION)
FROM memory_banks b
WHERE b.id=s.bank_id AND s.bank_id=(
  SELECT bank_id FROM memory_synthesis WHERE run_after<=NOW() AND lease_until<=NOW()
  ORDER BY run_after LIMIT 1 FOR UPDATE SKIP LOCKED)
RETURNING s.bank_id,s.connection_id,s.model,s.instructions,s.attempts,b.scope,b.name,b.description;
