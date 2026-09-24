--: Record()

--! run (p1, p2) : Record
UPDATE agent_deployments SET status='retired',retired_at=NOW(),token_hash=NULL WHERE id=:p1 AND agent_id=:p2 AND status='registered' RETURNING id;
