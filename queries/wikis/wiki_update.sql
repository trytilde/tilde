--! run (p1, p2?, p3?)
-- Absent fields keep their value; a name another wiki has updates nothing.
UPDATE wikis SET name=COALESCE(:p2,name),description=COALESCE(:p3,description)
WHERE id=:p1 AND NOT EXISTS (SELECT 1 FROM wikis o WHERE o.name=:p2 AND o.id<>:p1);
