--! run (p1, p2)
-- Ownership is fixed before any credential exists and never changes afterwards.
UPDATE connections SET owner_user_id=:p2 WHERE id=:p1 AND owner_user_id IS NULL AND status<>'ready';
