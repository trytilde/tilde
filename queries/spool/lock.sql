--: Record()
-- Serializes admission per queue for the rest of the transaction, so the capacity read and the
-- pointer insert that follow see every concurrent accept. Advisory locks span the database, so
-- the key includes the schema: installations sharing a database (and parallel tests, one schema
-- each) do not wait on each other's queues.
--! run (queue) : Record
SELECT pg_advisory_xact_lock(hashtextextended(current_schema() || '/' || :queue, 612345)) IS NULL AS locked;
