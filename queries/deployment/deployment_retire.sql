UPDATE agent_deployments SET status='retired',retired_at=NOW(),token_hash=NULL WHERE id=$1 AND agent_id=$2 AND status='registered' RETURNING id;
