--: Record()

--! run (p1, p2) : Record
UPDATE connections SET refresh_claim_until=now()+interval '90 seconds' WHERE id=:p1 AND status='ready' AND credential_version=:p2 AND (refresh_claim_until IS NULL OR refresh_claim_until<now()) RETURNING id;
