--! run (p1, p2)
UPDATE connections SET status = 'ready', token_expires_at = CASE WHEN :p2 THEN NOW() ELSE NULL END WHERE id = :p1;
