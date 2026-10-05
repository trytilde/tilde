--: Record()

-- Records a firing once; a repeated fire key returns no row.
--! run (id, routine, fire_key) : Record
INSERT INTO routine_runs(id,routine_id,fire_key) VALUES(:id,:routine,:fire_key)
ON CONFLICT (routine_id,fire_key) DO NOTHING RETURNING id;
