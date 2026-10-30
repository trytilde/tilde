--! run (p1)
UPDATE memory_synthesis SET run_after=NOW() WHERE bank_id=:p1;
