--: Record(last_run_at?)

--! run (p1) : Record
-- A bank's synthesis settings and how its worker is doing.
SELECT bank_id,connection_id,model,instructions,attempts,last_run_at,last_error
FROM memory_synthesis WHERE bank_id=:p1;
