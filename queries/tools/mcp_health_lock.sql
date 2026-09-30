--: Record()

--! run : Record
-- One MCP health sampler per database; rollback releases the lock.
SELECT pg_try_advisory_xact_lock(6073477207316124757) AS acquired;
