--! run (p1) : (failures)
-- Failures their content caused, across the batches from source start p1.
SELECT COALESCE(SUM(failures),0)::INTEGER AS failures FROM memory_synthesis_batches WHERE start_id=:p1;
