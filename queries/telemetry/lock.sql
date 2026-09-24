--! run
SELECT pg_advisory_xact_lock(78015491) IS NULL AS locked;
