--: Record()

--! run (p1, p2) : Record
UPDATE agents SET paused=:p2, generation=generation + CASE WHEN paused <> :p2 THEN 1 ELSE 0 END,
updated_at=NOW() WHERE id=:p1 AND deleted_at IS NULL
RETURNING generation;
