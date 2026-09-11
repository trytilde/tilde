WITH renamed AS (
 UPDATE connections SET name=$2,updated_at=NOW() WHERE id=$1 RETURNING id
)
UPDATE connection_setups SET action_id=$4,updated_at=NOW()
WHERE id=$3 AND connection_id IN (SELECT id FROM renamed);
