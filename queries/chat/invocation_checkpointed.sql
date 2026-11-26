--! run (p1)
-- The invocation saved a checkpoint (`suspend_run`).
UPDATE chat_invocations SET checkpointed = TRUE WHERE id = :p1;
