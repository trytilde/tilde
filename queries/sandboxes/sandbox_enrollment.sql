--! run (p1, p2)
-- A fresh enrollment token (digest p2) for the process about to be started. The current session
-- stays valid until the new process exchanges it.
UPDATE sandboxes SET enrollment_hash=:p2 WHERE id=:p1;
