-- Transaction-scoped: cancellation or any migration error releases the lock.
SELECT pg_advisory_xact_lock(hashtextextended(current_database() || ':' || current_schema(), 713456));
