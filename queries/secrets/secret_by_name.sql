--: Record()

-- The secret `name`: the team's own before the organization's.
--! run (p1) : Record
SELECT s.id FROM secrets s
WHERE s.name=:p1
LIMIT 1;
