--! run (p1, p2)
UPDATE chat_runs SET status=:p2 WHERE id=:p1 AND status IN ('active','waiting','suspending');
