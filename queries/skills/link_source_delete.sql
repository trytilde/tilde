--! run (connection_id, source_id)
DELETE FROM connection_skill_sources WHERE connection_id=:connection_id AND source_id=:source_id;
