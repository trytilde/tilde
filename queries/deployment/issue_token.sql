--: Record()

--! run (p1, p2, p3) : Record
UPDATE agent_deployments SET token_hash=:p2 WHERE id=:p1 AND agent_id=:p3 AND status='registered' RETURNING id;
