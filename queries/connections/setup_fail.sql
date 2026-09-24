--! run (p1, p2, p3)
UPDATE connection_setups SET step='failed',error_code=:p3,claimed_at=NULL,updated_at=now() WHERE id=:p1 AND action_id=:p2 AND step NOT IN ('complete','cancelled');
