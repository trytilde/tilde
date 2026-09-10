UPDATE connections SET status='ready',account_label=$2,token_expires_at=$3,credential_version=credential_version+1,refresh_claim_until=NULL,updated_at=now() WHERE id=$1;
