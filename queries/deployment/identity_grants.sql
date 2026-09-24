--: Record()

--! run (p1, p2) : Record
SELECT i.id,i.identity_type,i.value,(i.verified_at IS NOT NULL OR u.attested_at IS NOT NULL) AS verified,COALESCE(a.allowed,FALSE) AS allowed FROM chat_channel_identities i JOIN chat_users u ON u.id=i.id LEFT JOIN chat_channel_identity_access a ON a.connection_id=i.connection_id AND a.identity_id=i.id AND a.agent_id=:p2 WHERE i.connection_id=:p1;
