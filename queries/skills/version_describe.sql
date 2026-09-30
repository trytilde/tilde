-- Descriptions are read from SKILL.md; a better reader corrects them without a new version.
--! run (id, description)
UPDATE skill_versions SET description=:description WHERE id=:id;
