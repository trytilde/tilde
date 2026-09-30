--: Record()

--! run (id, source_id, name, source_path) : Record
INSERT INTO skills(id,source_id,name,source_path) VALUES(:id,:source_id,:name,:source_path)
ON CONFLICT (source_id,name) DO NOTHING RETURNING id;
