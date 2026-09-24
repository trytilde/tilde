--: Record()

--! run (p1) : Record
SELECT id FROM agents WHERE id=:p1 AND deleted_at IS NULL FOR SHARE;
