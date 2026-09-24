--! run (p1, p2)
UPDATE chat_runs SET status='suspending' WHERE id=:p1 AND status IN ('active','suspending') AND EXISTS(SELECT 1 FROM chat_invocations WHERE id=:p2 AND run_id=:p1 AND status='running');
