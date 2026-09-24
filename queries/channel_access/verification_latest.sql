--: Record()

--! run (p1, p2) : Record
SELECT id,status,created_at FROM chat_identity_verifications WHERE identity_id=:p1 AND agent_id=:p2 ORDER BY created_at DESC LIMIT 1;
