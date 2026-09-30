--: Record()

-- Serialises syncs and edits of one source, returning the latest sync to have started.
--! run (id) : Record
SELECT sync_generation FROM skill_sources WHERE id=:id FOR UPDATE;
