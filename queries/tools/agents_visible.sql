--: Record()

--! run : Record
SELECT id,name FROM agents WHERE deleted_at IS NULL AND NOT paused ORDER BY name,id LIMIT 500;
