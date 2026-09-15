SELECT id,concurrency_policy FROM agents WHERE id=$1 AND NOT paused AND deleted_at IS NULL FOR SHARE;
