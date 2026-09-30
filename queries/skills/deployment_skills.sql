--: Record()

-- The bundled skill versions a deployment shipped.
--! run (deployment_id) : Record
SELECT s.id AS skill_id,s.name,v.id AS version_id,v.number,v.description,ds.origin
FROM deployment_skills ds JOIN skill_versions v ON v.id=ds.version_id JOIN skills s ON s.id=v.skill_id
WHERE ds.deployment_id=:deployment_id ORDER BY s.name;
