--: Record(verified_at?, attested_at?)

--! run (p1, p2) : Record
SELECT i.id,i.value,i.identity_type,i.verified_at,u.attested_at FROM chat_channel_identities i JOIN chat_users u ON u.id=i.id WHERE i.connection_id=:p1 AND i.id=:p2;
