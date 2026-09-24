--! run (id)
INSERT INTO root_identities(id) VALUES(:id) ON CONFLICT DO NOTHING;
