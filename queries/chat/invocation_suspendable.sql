--! run (p1)
-- The invocation made a call it can suspend on (see `runs_suspend_due.sql`).
UPDATE chat_invocations SET suspendable = TRUE WHERE id = :p1 AND NOT suspendable;
