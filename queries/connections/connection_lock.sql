--! run (p1)
SELECT pg_advisory_xact_lock(hashtextextended(:p1, 812345)) IS NULL AS locked;
