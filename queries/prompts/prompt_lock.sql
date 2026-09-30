--: Record()

--! run (p1) : Record
SELECT id FROM prompts WHERE id=:p1 FOR UPDATE;