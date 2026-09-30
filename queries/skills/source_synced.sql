-- A failed sync keeps the commit and skills of the last good one. Only the latest sync to have
-- started records its outcome.
--! run (id, generation, commit_sha?, sync_error?)
UPDATE skill_sources SET synced_at=NOW(),sync_error=:sync_error,commit_sha=COALESCE(:commit_sha,commit_sha),updated_at=NOW() WHERE id=:id AND sync_generation=:generation;
