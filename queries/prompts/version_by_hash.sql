--: Record()

--! run (p1, p2) : Record
SELECT id FROM prompt_versions WHERE prompt_id=:p1 AND hash=:p2;