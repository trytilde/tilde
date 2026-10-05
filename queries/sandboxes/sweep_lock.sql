--: Record()

--! run : Record
-- One sandbox sweeper per database; rollback releases the lock.
SELECT pg_try_advisory_xact_lock(4471205839260136011) AS acquired;
