--! run (p1)
-- An expired invocation that had saved the checkpoint it was asked for ended as a suspension.
UPDATE chat_invocations SET status = 'stopped' WHERE id = :p1 AND status = 'failed';
