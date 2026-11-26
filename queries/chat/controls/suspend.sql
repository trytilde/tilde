--! run (p1, p2, p3)
-- The run moves to `suspending` while the invocation runs; the first reason asked is kept.
WITH run AS (
    UPDATE chat_runs SET status='suspending' WHERE id=:p1 AND status IN ('active','suspending')
      AND EXISTS(SELECT 1 FROM chat_invocations WHERE id=:p2 AND run_id=:p1 AND status='running')
    RETURNING id
)
UPDATE chat_invocations SET suspended_for=COALESCE(suspended_for, :p3)
WHERE id=:p2 AND EXISTS(SELECT 1 FROM run);
