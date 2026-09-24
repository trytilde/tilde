--: Record()

--! run (p1) : Record
SELECT id,concurrency_policy FROM agents WHERE id=:p1 AND NOT paused AND deleted_at IS NULL FOR SHARE;
