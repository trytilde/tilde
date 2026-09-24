--: Record()

--! run (p1, p2, p3) : Record
SELECT id FROM agent_deployments WHERE agent_id=:p1 AND status='registered' AND target=ANY(:p2) AND id<>:p3 ORDER BY created_at DESC,id LIMIT 1;
