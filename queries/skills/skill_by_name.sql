--: Record()

--! run (source_id, name) : Record
SELECT id FROM skills WHERE source_id=:source_id AND name=:name;
