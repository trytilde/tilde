UPDATE connections SET refresh_claim_until=NULL WHERE id=$1 AND credential_version=$2;
