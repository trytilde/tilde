--: Record()

--! run (p1, p2) : Record
UPDATE chat_identity_verifications SET status='approved' WHERE id=:p1 AND token_hash=:p2 AND status='delivered' AND expires_at>NOW() RETURNING connection_id,agent_id,identity_id;
