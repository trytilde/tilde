--: Record(verified_at?)

--! run (p1, p2, p3, p4) : Record
INSERT INTO chat_channel_identities(id,connection_id,identity_type,value) VALUES(:p1,:p2,:p3,:p4)
ON CONFLICT(connection_id,identity_type,value) DO UPDATE SET value=excluded.value RETURNING id,value,identity_type,verified_at;
