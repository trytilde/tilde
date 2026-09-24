--: Record()

--! run (p1, p2, p3?) : Record
UPDATE connections SET token_expires_at=:p3,credential_version=credential_version+1,refresh_claim_until=NULL,updated_at=now() WHERE id=:p1 AND credential_version=:p2 AND status='ready' RETURNING id;
