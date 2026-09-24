--! run (p1)
UPDATE chat_invocations SET lease_expires_at=NOW()+INTERVAL '30 seconds' WHERE id=:p1 AND status='running';
