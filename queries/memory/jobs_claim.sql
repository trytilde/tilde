--! run (p1, p2) : (memory_id, requested_at, attempts, title, content)
-- Claim up to p1 due jobs, leasing them for p2 seconds.
UPDATE memory_embedding_jobs j SET run_after=NOW()+make_interval(secs=>:p2::DOUBLE PRECISION),attempts=j.attempts+1
FROM memories m
WHERE m.id=j.memory_id AND j.memory_id IN (
  SELECT memory_id FROM memory_embedding_jobs WHERE run_after<=NOW() ORDER BY run_after
  LIMIT :p1 FOR UPDATE SKIP LOCKED)
RETURNING j.memory_id,j.requested_at,j.attempts,m.title,m.content;
