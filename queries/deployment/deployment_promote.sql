UPDATE agent_deployment_settings s SET serving_deployment_id=d.id FROM agent_deployments d
WHERE s.agent_id=$1 AND d.id=$2 AND d.agent_id=$1 AND d.status='registered' RETURNING d.target,d.endpoint_url;
