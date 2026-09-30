--! run (id)
UPDATE chat_users SET attested_at=NOW()
WHERE id=:id AND attested_at IS NULL;
