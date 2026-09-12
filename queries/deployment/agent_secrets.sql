SELECT a.generation,a.webhook_signing_key FROM agents a WHERE a.id=$1 AND a.deleted_at IS NULL;
