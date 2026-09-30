--! run (connection_id, source_id)
INSERT INTO connection_skill_sources(connection_id,source_id) VALUES(:connection_id,:source_id) ON CONFLICT DO NOTHING;
