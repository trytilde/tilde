--: Record()

--! run (p1) : Record
SELECT a.generation FROM agents a WHERE a.id=:p1 AND a.deleted_at IS NULL;
