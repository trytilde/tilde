--! run (p1?)
-- Ask for one memory's embedding (p1), or every current memory's.
INSERT INTO memory_embedding_jobs(memory_id)
SELECT m.id FROM current_memories m WHERE :p1::UUID IS NULL OR m.id=:p1
ON CONFLICT(memory_id) DO UPDATE SET requested_at=clock_timestamp(),run_after=NOW(),attempts=0;
