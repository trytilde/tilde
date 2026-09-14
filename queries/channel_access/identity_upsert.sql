INSERT INTO chat_channel_identities(id,connection_id,identity_type,value) VALUES($1,$2,$3,$4)
ON CONFLICT(connection_id,identity_type,value) DO UPDATE SET value=excluded.value RETURNING id,value,identity_type,verified_at;
