--: Record()

-- Claims the next sync generation before fetching.
--! run (id) : Record
UPDATE skill_sources SET sync_generation=sync_generation+1 WHERE id=:id RETURNING sync_generation;
