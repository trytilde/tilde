--! run (p1, p2)
UPDATE connections SET status='requires_action',refresh_claim_until=NULL,updated_at=now()
WHERE id=:p1 AND credential_version=:p2 AND status='ready';
