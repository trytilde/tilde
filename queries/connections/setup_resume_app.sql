--! run (p1, p2)
UPDATE connection_setups SET step=:p2 WHERE id=:p1 AND step='fields' AND claimed_at IS NULL;
