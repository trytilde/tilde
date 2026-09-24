--! run (p1, p2?, p3?)
UPDATE connections SET status='ready',account_label=:p2,token_expires_at=:p3,credential_version=credential_version+1,refresh_claim_until=NULL,updated_at=now() WHERE id=:p1;
