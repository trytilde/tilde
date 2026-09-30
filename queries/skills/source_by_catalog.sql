--: Record()

--! run (catalog_group) : Record
SELECT id FROM skill_sources WHERE catalog_group=:catalog_group;
