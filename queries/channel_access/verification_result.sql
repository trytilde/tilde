--! run (p1, p2)
UPDATE chat_identity_verifications SET status=:p2 WHERE id=:p1 AND status='pending';
