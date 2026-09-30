--! run (id)
UPDATE skills SET updated_at=NOW() WHERE id=:id;
