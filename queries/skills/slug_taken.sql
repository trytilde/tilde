--: Record()

--! run (slug) : Record
SELECT EXISTS(SELECT 1 FROM skill_sources WHERE slug=:slug) AS taken;
