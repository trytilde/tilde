UPDATE connections SET status='disconnected',token_expires_at=NULL,credential_version=credential_version+1,updated_at=now() WHERE id=$1;
