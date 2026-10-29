--! run (p1, p2, p3, p4, p5)
-- Store the embedding only if the job was not asked again since it was claimed.
WITH done AS (DELETE FROM memory_embedding_jobs WHERE memory_id=:p1 AND requested_at=:p2 RETURNING memory_id)
UPDATE memories SET embedding=:p3::TEXT::halfvec,embedding_dims=:p4,embedding_version=:p5
WHERE id IN (SELECT memory_id FROM done);
