--: Record()

--! run (p1) : Record
SELECT pg_try_advisory_xact_lock(hashtextextended(:p1::UUID::TEXT, 713455)) AS locked;
