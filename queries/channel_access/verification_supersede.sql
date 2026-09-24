--! run (p1, p2)
UPDATE chat_identity_verifications SET expires_at=NOW() WHERE identity_id=:p1 AND agent_id=:p2 AND status<>'approved';
