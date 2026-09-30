--: Record(deployment_id?, commit_sha?)

--! run (p1) : Record
SELECT v.id,v.prompt_id,v.number,v.hash,v.template,v.config,v.variables,v.format,v.origin,v.deployment_id,d.commit_sha,v.created_at
FROM prompt_versions v LEFT JOIN agent_deployments d ON d.id=v.deployment_id
WHERE v.id=ANY(:p1::UUID[]) ORDER BY v.number DESC;
