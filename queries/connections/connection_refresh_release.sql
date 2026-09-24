--! run (p1, p2)
UPDATE connections SET refresh_claim_until=NULL WHERE id=:p1 AND credential_version=:p2;
