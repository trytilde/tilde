WITH expired AS (
    DELETE FROM connection_setups
    WHERE created_at<=now()-interval '10 minutes' OR expires_at<=now()
    RETURNING connection_id
)
UPDATE connections SET status='failed',updated_at=now()
WHERE id IN (SELECT connection_id FROM expired)
AND status NOT IN ('ready','disconnected');
