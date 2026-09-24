--: Record(target_reference?, repository?, commit_sha?, external_id?, retired_at?, commit_message?, branch?, commit_author?)

--! run (p1) : Record
SELECT d.id,d.agent_id,d.source,d.target,d.target_reference,d.repository,d.commit_sha,d.external_id,d.label,d.status,d.token_hash IS NOT NULL AS token_issued,
 CASE WHEN s.routing='weighted' THEN d.status='registered' AND d.traffic_weight>0 ELSE COALESCE(d.id=s.serving_deployment_id,false) END AS serving,d.created_at,d.retired_at,d.commit_message,d.branch,d.commit_author,d.traffic_weight
FROM agent_deployments d JOIN agent_deployment_settings s ON s.agent_id=d.agent_id WHERE d.agent_id=:p1 ORDER BY d.created_at DESC,d.id;
