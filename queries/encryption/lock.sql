-- Serialize first-time key creation across server processes.
SELECT pg_advisory_xact_lock($1) AS "locked!: ()";
