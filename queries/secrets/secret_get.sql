--: Record()

--! run (p1) : Record
SELECT s.id,s.name
FROM secrets s WHERE s.id=:p1;
