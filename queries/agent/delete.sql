--! run (p1)
UPDATE agents SET deleted_at=NOW(), capabilities='{}'::jsonb, updated_at=NOW() WHERE id=:p1 AND paused AND deleted_at IS NULL;
