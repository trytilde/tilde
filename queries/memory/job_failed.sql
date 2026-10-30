--! run (p1, p2, p3)
-- Retry after p3 seconds, unless the job was asked again since it was claimed.
UPDATE memory_embedding_jobs SET run_after=NOW()+make_interval(secs=>:p3::DOUBLE PRECISION)
WHERE memory_id=:p1 AND requested_at=:p2;
