--: Record()

--! run (p1, p2, p3, p4) : Record
UPDATE connection_setups SET callback_token=:p3,callback_hash=:p4 WHERE id=:p1 AND action_id=:p2 AND claimed_at IS NOT NULL AND step NOT IN ('complete','failed','cancelled') RETURNING id;
