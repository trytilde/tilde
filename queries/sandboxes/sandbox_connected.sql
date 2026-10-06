--! run (p1)
UPDATE sandboxes SET connected_at=NOW() WHERE id=:p1;
