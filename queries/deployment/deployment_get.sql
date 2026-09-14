SELECT d.id,d.agent_id,d.source,d.target,d.endpoint_url,d.target_reference,d.repository,d.commit_sha,d.external_id,d.label,d.status,d.token_hash IS NOT NULL AS "token_issued!",
 d.id=s.serving_deployment_id AS "serving!",d.created_at,d.retired_at
FROM agent_deployments d JOIN agent_deployment_settings s ON s.agent_id=d.agent_id WHERE d.id=$1 AND d.agent_id=$2;
