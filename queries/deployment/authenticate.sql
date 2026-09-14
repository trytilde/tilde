SELECT d.agent_id,d.id AS deployment_id,d.target FROM agent_deployments d JOIN agents a ON a.id=d.agent_id WHERE d.token_hash=$1 AND d.status='registered' AND a.deleted_at IS NULL;
