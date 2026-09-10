UPDATE connections SET status='requires_action',refresh_claim_until=NULL,updated_at=now()
WHERE id=$1 AND credential_version=$2 AND status='ready';
