--: Record()

--! run (p1) : Record
SELECT EXISTS(SELECT 1 FROM agents WHERE id=:p1) AS exists;
