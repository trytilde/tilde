--! run (p1, p2, p3, p4)
-- A change starts the bank afresh: due now, no failures counted.
INSERT INTO memory_synthesis(bank_id,connection_id,model,instructions) VALUES(:p1,:p2,:p3,:p4)
ON CONFLICT(bank_id) DO UPDATE SET connection_id=EXCLUDED.connection_id,
  model=EXCLUDED.model,instructions=EXCLUDED.instructions,run_after=NOW(),attempts=0,last_error='';
