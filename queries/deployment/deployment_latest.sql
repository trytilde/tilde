SELECT id FROM agent_deployments WHERE agent_id=$1 AND status='registered' AND target=ANY($2) ORDER BY created_at DESC,id LIMIT 1;
