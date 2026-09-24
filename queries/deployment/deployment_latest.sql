--: Record()

--! run (p1, p2) : Record
SELECT id FROM agent_deployments WHERE agent_id=:p1 AND status='registered' AND target=ANY(:p2) ORDER BY created_at DESC,id LIMIT 1;
