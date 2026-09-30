-- Skills a sync no longer finds go, with their versions and assignments.
--! run (source_id, names)
DELETE FROM skills WHERE source_id=:source_id AND NOT (name=ANY(:names::TEXT[]));
