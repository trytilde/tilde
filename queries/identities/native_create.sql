--! run (id, value)
INSERT INTO chat_native_identities(id,value) VALUES(:id,:value) ON CONFLICT DO NOTHING;
