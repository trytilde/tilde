--: Record()
-- Serializes admission per queue for the rest of the transaction, so the capacity read and the
-- pointer insert that follow see every concurrent accept.
--! run (queue) : Record
SELECT pg_advisory_xact_lock(hashtextextended(:queue, 612345)) IS NULL AS locked;
