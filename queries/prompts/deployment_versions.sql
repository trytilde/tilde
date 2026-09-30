--: Record()

-- The prompt versions a deployment shipped, by prompt name.
--! run (deployment_id) : Record
SELECT v.id AS version_id,p.id AS prompt_id,p.name
FROM deployment_prompts dp JOIN prompt_versions v ON v.id=dp.version_id JOIN prompts p ON p.id=v.prompt_id
WHERE dp.deployment_id=:deployment_id ORDER BY p.name;
