--! run (p1)
-- Serialize first-time key creation across server processes.
SELECT pg_advisory_xact_lock(:p1) IS NULL AS locked;
