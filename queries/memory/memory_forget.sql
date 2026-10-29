--! run (p1)
WITH gone AS (DELETE FROM memory_embedding_jobs WHERE memory_id=:p1)
UPDATE memories SET deleted_at=NOW(),updated_at=NOW() WHERE id=:p1 AND deleted_at IS NULL;
