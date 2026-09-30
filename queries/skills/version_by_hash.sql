--: Record()

--! run (skill_id, hash) : Record
SELECT id FROM skill_versions WHERE skill_id=:skill_id AND hash=:hash ORDER BY number LIMIT 1;
