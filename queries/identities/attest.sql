--! run (id, user_id?, api_key_id?)
UPDATE chat_users SET attested_at=NOW(),attested_by_user_id=:user_id,attested_by_api_key_id=:api_key_id
WHERE id=:id AND attested_at IS NULL;
