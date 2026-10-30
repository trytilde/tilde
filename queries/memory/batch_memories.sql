--! run (p1) : (batch_id, memory_id, title, forgotten)
-- The memories batches p1 wrote, and those they forgot.
SELECT DISTINCT e.batch_id,m.id AS memory_id,m.title,FALSE AS forgotten
FROM memory_evidence e JOIN memories m ON m.id=e.memory_id WHERE e.batch_id=ANY(:p1::UUID[])
UNION
SELECT r.batch_id,r.memory_id,r.title,TRUE FROM memory_revisions r
WHERE r.batch_id=ANY(:p1::UUID[]) AND r.operation='forget'
ORDER BY 1,4,3;
