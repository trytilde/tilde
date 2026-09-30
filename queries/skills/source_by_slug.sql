--: Record()

--! run (slug) : Record
SELECT id,kind FROM skill_sources WHERE slug=:slug AND kind<>'bundled';
