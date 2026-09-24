--! run (p1, p2, p3, p4)
WITH renamed AS (
 UPDATE connections SET name=:p2,updated_at=NOW() WHERE id=:p1 RETURNING id
)
UPDATE connection_setups SET action_id=:p4,updated_at=NOW()
WHERE id=:p3 AND connection_id IN (SELECT id FROM renamed);
