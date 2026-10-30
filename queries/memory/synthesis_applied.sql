--! run (p1) : (applied)
-- Whether batch p1 is done: applied or skipped.
SELECT EXISTS (SELECT 1 FROM memory_synthesis_batches WHERE id=:p1 AND status<>'failed') AS applied;
