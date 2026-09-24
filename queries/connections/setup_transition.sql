--: Record()

--! run (p1, p2, p3, p4) : Record
UPDATE connection_setups SET step=:p3,action_id=:p4,claimed_at=NULL,error_code=NULL,updated_at=now() WHERE id=:p1 AND action_id=:p2 AND claimed_at IS NOT NULL RETURNING id;
