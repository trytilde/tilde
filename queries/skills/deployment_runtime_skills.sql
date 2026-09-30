--: Record()

-- A deployment's bundled skills in the shape of runtime_skills.
--! run (deployment_id) : Record
SELECT s.id,s.name,src.slug AS source_slug,v.id AS version_id,v.description
FROM deployment_skills ds JOIN skill_versions v ON v.id=ds.version_id JOIN skills s ON s.id=v.skill_id
JOIN skill_sources src ON src.id=s.source_id
WHERE ds.deployment_id=:deployment_id ORDER BY s.name;
