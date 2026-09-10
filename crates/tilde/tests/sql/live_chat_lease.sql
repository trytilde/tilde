UPDATE chat_invocations SET status = 'running', lease_expires_at = NOW() + INTERVAL '10 minutes' WHERE id = $1
