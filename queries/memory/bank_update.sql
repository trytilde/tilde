--! run (p1, p2?, p3?)
-- Absent fields keep their value; a name another bank has updates nothing.
UPDATE memory_banks SET name=COALESCE(:p2,name),description=COALESCE(:p3,description)
WHERE id=:p1 AND NOT EXISTS (SELECT 1 FROM memory_banks o WHERE o.name=:p2 AND o.id<>:p1);
