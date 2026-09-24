--! run (p1)
UPDATE chat_channel_identities SET verified_at=COALESCE(verified_at,NOW()) WHERE id=:p1;
