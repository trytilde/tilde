UPDATE chat_invocations SET status='running',started_at=COALESCE(started_at,NOW()),lease_expires_at=NOW()+INTERVAL '30 seconds' WHERE id=$1 AND status IN ('pending','running') RETURNING id;
