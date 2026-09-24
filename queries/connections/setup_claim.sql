--: Record()

--! run (p1, p2, p3) : Record
UPDATE connection_setups SET claimed_at=now(),updated_at=now() WHERE id=:p1 AND action_id=:p2 AND step=:p3 AND claimed_at IS NULL AND expires_at>now() RETURNING id;
