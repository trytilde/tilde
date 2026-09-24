--! run (p1, p2, p3?)
UPDATE connection_setups SET provider_redirect_url=:p3 WHERE id=:p1 AND action_id=:p2 AND claimed_at IS NOT NULL AND step NOT IN ('complete','failed','cancelled');
