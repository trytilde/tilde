--! run (p1, p2)
-- Used by invocation p2.
UPDATE sandboxes SET last_used_at=NOW(),invocation_id=:p2 WHERE id=:p1;
