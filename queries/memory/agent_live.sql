--! run (p1) : (live)
SELECT EXISTS (SELECT 1 FROM agents WHERE id=:p1 AND deleted_at IS NULL) AS live;
