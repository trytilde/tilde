--! run (p1, p2)
UPDATE chat_invocations SET suspend_at = :p2 WHERE id = :p1;
