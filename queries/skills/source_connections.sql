--: Record()

--! run (source_id) : Record
SELECT connection_id FROM connection_skill_sources WHERE source_id=:source_id ORDER BY created_at;
