UPDATE connections SET status = 'ready', token_expires_at = CASE WHEN $2 THEN NOW() ELSE NULL END WHERE id = $1
